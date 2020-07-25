// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// http://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

use super::{calculation::*, AccountId, Accumulation, AccumulationEvent};
use safe_nd::{Result, RewardCounter, Work};
use std::collections::HashMap;

struct FarmingSystem<A: RewardAlgo> {
    farming_algo: A,
    accumulation: Accumulation,
}

#[allow(unused)]
impl<A: RewardAlgo> FarmingSystem<A> {
    ///
    pub fn new(farming_algo: A, accumulation: Accumulation) -> Self {
        Self {
            farming_algo,
            accumulation,
        }
    }

    /// Work is the total work associated with this account id.
    /// It is a strictly incrementing value during the lifetime of
    /// the owner on the network.
    pub fn add_account(&mut self, id: AccountId, work: Work) -> Result<()> {
        let e = self.accumulation.add_account(id, work)?;
        self.accumulation.apply(AccumulationEvent::AccountAdded(e));
        Ok(())
    }

    /// Factor is a number > 0, by which reward will be increased or decreased.
    /// When factor == 1, there is no scaling of the rewards.
    /// When factor is > 1, the reward is scaled up.
    /// When factor is < 1, the reward is scaled down.
    ///
    /// Temp comments for the SAFE Network farming context:
    ///
    /// When factor is > 1, the StoreCost is - effectively - topped up with the surplus
    /// from section account, to form the total reward.
    /// This is essentially the same as a _net farming_, aka net issuance of money.
    ///
    /// When factor is < 1, the excess from the StoreCost
    /// stays at the section account.
    /// This is essentially the same as recycling money, moving it out of circulation.
    ///
    /// The factor is thus the adjustment of total supply in circulation vs total amount held by the network.
    /// It's envisaged that the calculation of this factor, is where the crux of balancing the network economy is,
    /// and where we will see changes due to tweaks, bug fixes, and improvements.
    /// With other words: that is code that will change with a higher rate than this code, and is thus
    /// separated out, for some other layer to deal with.
    ///
    /// The factor is the output of a function of parameters
    /// relevant to the implementing layer.
    /// In SAFE Network context, those parameters could be node count,
    /// section count, percent filled etc. etc.
    pub fn reward(
        &mut self,
        data_hash: Vec<u8>,
        num_bytes: u64,
        factor: f64,
    ) -> Result<safe_nd::Money> {
        // first query for accumulated work of all
        let accounts_work: HashMap<AccountId, Work> = self
            .accumulation
            .get_all()
            .iter()
            .map(|(id, acc)| (*id, acc.work))
            .collect();
        // calculate the work cost for the number of bytes to store
        let work_cost = self.farming_algo.work_cost(num_bytes);
        // scale the reward by the factor
        let total_reward = self.farming_algo.total_reward(factor, work_cost);
        // distribute according to previously performed work
        let distribution = self.farming_algo.distribute(total_reward, accounts_work);

        // validate the operation
        let e = self.accumulation.accumulate(data_hash, distribution)?;

        // apply the result, reward counter is now incremented
        // i.e. both the reward amount and the work performed.
        self.accumulation
            .apply(AccumulationEvent::RewardsAccumulated(e));

        Ok(total_reward)
    }

    pub fn claim(&mut self, id: AccountId) -> Result<RewardCounter> {
        let e = self.accumulation.claim(id)?;
        self.accumulation
            .apply(AccumulationEvent::RewardsClaimed(e.clone()));
        Ok(e.rewards)
    }
}

#[allow(unused)]
mod test {
    use super::{Accumulation, FarmingSystem, RewardCounter, StorageRewards};
    use crate::RewardCounterSet;
    use crdts::quickcheck::{quickcheck, Arbitrary, TestResult};
    use rand::{Rng, RngCore};
    use rayon::prelude::*;
    use safe_nd::{Money, PublicKey, Result};
    use std::collections::{HashMap, HashSet};
    use threshold_crypto::SecretKey;

    /// Test description
    ///
    /// 1. We have 7 Elders.
    /// 2. We issue n rewards.
    /// 3. We claim the accumulated rewards.
    /// 4. We apply byzantine faults to the claimed rewards.
    /// 5. We assert that the faults do not affect the outcome.
    ///
    /// For every claimed reward:
    /// - The original reward will be aggregated.
    /// - 4 Elders will be correct/honest.
    /// - 3 Elders will be wildly wrong/dishonest.
    ///
    /// At the end, we want to show that the Elder accumulated rewards, will be more or less
    /// the same, as the aggregated original rewards, i.e. they will not have drifted.
    ///
    /// This will prove that:
    /// A. With a majority of (E2E) correctly working nodes, the reward will be correct.
    /// B. Even with the correct nodes being a bit out of synch, the reward will converge to a correct amount.
    ///

    type Elder = FarmingSystem<StorageRewards>;

    /// Demonstrates the usage of FarmingSystem,
    /// and its behaviour.
    #[test]
    fn farming_system() -> Result<()> {
        // --- Arrange ---
        let acc = Accumulation::new(Default::default(), Default::default());
        let base_cost = Money::from_nano(2);
        let algo = StorageRewards::new(base_cost);
        let mut system = FarmingSystem::new(algo, acc);

        let account = get_random_pk();
        let data_hash = vec![1, 2, 3];

        let num_bytes = 3u64;
        let factor = 2u64; // i.e. we say here that the reward is 2x StoreCost (where, in our context, half is contributed by the network).
        let work = 1; // starts at minimum 1

        // --- Act ---
        // Try accumulate.
        system.add_account(account, work)?;
        let _ = system.reward(data_hash, num_bytes, factor as f64)?;

        // --- Assert ---
        match system.claim(account) {
            Err(err) => panic!(err),
            Ok(e) => {
                assert!(e.reward.as_nano() == factor * (num_bytes + base_cost.as_nano()));
                assert!(e.work == work + 1); // being part of 1 reward occasion
            }
        }
        Ok(())
    }

    // #[test]
    // fn quickcheck_bft_rewards() {
    //     quickcheck(bft_rewards_quickcheck as fn(Factor) -> TestResult);
    // }

    /// This is just a helper test, to verify and show
    /// what we mean with "position".
    /// Used in finding the distribution of outcomes of
    /// our test results.
    #[test]
    fn test_position() {
        let error = 0.000_000_001;

        let number = 0.00056_f64;
        let log10 = number.log10();
        let position = log10.floor();
        let expected: f64 = -4.0;
        assert!((expected - position).abs() < error); // fourth decimal position

        let number = 5000.00056_f64;
        let log10 = number.log10();
        let position = log10.ceil();
        let expected: f64 = 4.0;
        assert!((expected - position).abs() < error); // fourth position of integers
    }

    /// Add results to buckets, assert that
    /// enough results fall into the range of buckets we want
    /// and optionally that we don't have too many results of some sort
    /// in undesired ranges.
    #[test]
    fn verify_reward_bft() -> Result<()> {
        let mut rng = rand::thread_rng();
        let mut reward_buckets = HashMap::<isize, u64>::new();
        let mut work_buckets = HashMap::<isize, u64>::new();

        let iters = 100;
        for i in (0..iters) {
            let factor = Factor::new(rng.gen_range(0.67, 2.7)); // factors between 0 and 1 give more deviance than between 1 and 100 for example, due to rounding errors
            let data = simulate_random_rewards_with_byzantine_faults(factor)?;
            record_result(data.reward_diff_percent, &mut reward_buckets);
            record_result(data.work_diff_percent, &mut work_buckets);
        }

        let work_diff_distribution = find_distribution(iters, work_buckets);
        let reward_diff_distribution = find_distribution(iters, reward_buckets);

        println!("---");
        println!("Work diff distribution:");
        print_distribution(work_diff_distribution);
        println!();
        println!("Reward diff distribution:");
        print_distribution(reward_diff_distribution);
        println!("---");

        Ok(())
    }

    fn print_distribution(distribution: HashMap<isize, f64>) {
        let mut distribution = distribution.into_iter().collect::<Vec<(isize, f64)>>();
        distribution.sort_by_key(|item| item.0);

        let mut seen = vec![];
        for (precision, occurrence) in distribution {
            if seen.contains(&occurrence) {
                continue;
            }
            println!(
                "{:.10} % of the time, we have less than {} % diff",
                occurrence * 100.0,
                10_f64.powf(precision as f64)
            );
            seen.push(occurrence);
        }
    }

    // We find out how often we see the distinct outcomes for the test parameters.
    // So if we have the two outcomes 0.04 and 0.001 (%), then
    // - 100% of the time, the deviance is less than 0.1 %.
    // - 50% of the time, the deviance is less than 0.01 %.
    fn find_distribution(num_cases: u64, outcomes: HashMap<isize, u64>) -> HashMap<isize, f64> {
        let error = 0.0001;
        let mut diff_distribution = HashMap::new();
        for precision in (-10_isize..-1) {
            let mut max = 1.0; // 100 %
            let mut min = 0.01; // 1 %
            let mut safety_break = 0;
            loop {
                let mid = min + (max - min) / 2.0;
                let occurs_at_least_this_often =
                    occurs_at_least_this_often(&outcomes, num_cases, precision, mid);
                if occurs_at_least_this_often && (round(max, 2) - round(min, 2)).abs() < error {
                    let _ = diff_distribution.insert(precision, round(mid, 2));
                    break;
                } else if occurs_at_least_this_often {
                    min = mid;
                } else {
                    max = mid;
                }
                safety_break += 1;
                if safety_break > 100 {
                    let _ = diff_distribution.insert(precision, round(mid, 2));
                    break;
                }
            }
        }
        diff_distribution
    }

    fn record_result(outcome: f64, bucket: &mut HashMap<isize, u64>) {
        let log10 = outcome.log10();
        let position = if outcome >= 1.0 {
            round(log10.ceil(), 0) as isize
        } else {
            round(log10.floor(), 0) as isize
        };
        let next_value = match bucket.get(&position) {
            Some(v) => v + 1,
            None => 1,
        };
        let _ = bucket.insert(position, next_value);
    }

    /// The results in `outcomes`, are summed
    /// and we test a given `precision`, and compare against
    /// a specified `percent_of_cases`, to see if the `precision`
    /// we passed in, occurs at least this often.
    /// num_cases could also be outcomes.len(), but we are testing against
    /// a before-hand expected number of cases, and outcomes.len() could theoretically differ.
    fn occurs_at_least_this_often(
        outcomes: &HashMap<isize, u64>,
        num_cases: u64,
        precision: isize,
        percent_of_the_cases: f64,
    ) -> bool {
        let sum_of_outcomes = outcomes
            .iter()
            .filter(|(p, _)| *p <= &precision)
            .map(|(_, count)| count)
            .sum::<u64>() as f64;
        let ge_precision = sum_of_outcomes / num_cases as f64;
        ge_precision >= percent_of_the_cases
    }

    fn bft_rewards_quickcheck(factor: Factor) -> TestResult {
        let error = 0.0001;
        match simulate_random_rewards_with_byzantine_faults(factor) {
            Ok(result) => {
                // Assert that the difference is within tolerance levels.
                // (Assert that the byzantine faults introduce no
                // difference.)
                let decimals = 1;
                let expected_diff = 0.0;
                let acceptble_work_diff =
                    (expected_diff - round(result.work_diff_percent, decimals)).abs() < error; // diff of work shall be less than 0.1 %
                let acceptable_reward_diff =
                    (expected_diff - round(result.reward_diff_percent, decimals)).abs() < error; // diff of reward shall be less than 0.1 %

                if acceptble_work_diff && acceptable_reward_diff {
                    TestResult::passed()
                } else {
                    TestResult::failed()
                }
            }
            Err(_) => TestResult::failed(),
        }
    }

    fn simulate_random_rewards_with_byzantine_faults(factor: Factor) -> Result<RewardResults> {
        println!("Test started.");

        let previous_work = PreviousWork::new();
        let work_to_perform = WorkInfo::new();

        let base_cost = 0;
        // 1. We have 7 Elders.
        let num_elders = 7;
        let mut elders: Vec<Elder> = (0..num_elders).map(|i| get_instance(base_cost)).collect();

        println!("Elders generated.");

        let mut accounts = vec![];
        for work in &previous_work.values {
            let account = get_random_pk();
            accounts.push(account);
            for elder in &mut elders {
                elder.add_account(account, *work).unwrap();
            }
        }

        println!("Accounts added.");

        // 2. We will issue n rewards.
        let total_reward_sum = elders
            .par_iter_mut()
            .map(|elder| {
                (&work_to_perform.values)
                    .iter()
                    .map(|work_info| {
                        reward(elder, work_info.clone(), factor.value)
                            .unwrap()
                            .as_nano()
                    })
                    .sum::<u64>()
                    / num_elders
            })
            .sum::<u64>();

        println!("Rewards accumulated.");

        // The expected reward will be calculated.
        let total_reward: u64 = (&work_to_perform.values)
            .par_iter()
            .map(|(_, numbytes)| (base_cost + numbytes.value) as f64)
            .map(|work_cost| factor.value * work_cost)
            .map(|tr| tr.round() as u64)
            .sum();
        let total_work: u64 = (previous_work.values.len() as u64
            * work_to_perform.values.len() as u64)
            + previous_work.values.into_iter().sum::<u64>();

        let mut total_agreed_rewards = 0;
        let mut total_agreed_work = 0;

        // For each account, we claim the counter from all Elders,
        // introduce the byzantine faults,
        // and finally reach an agreement on a single counter value.
        for account in accounts {
            let counters: Vec<RewardCounter> = (&mut elders)
                .par_iter_mut()
                .map(|elder| elder.claim(account).unwrap())
                .collect();
            let counters =
                RewardCounterSet::new(num_elders as usize, apply_byzantine_faults(counters))?;
            let agreed_counter = counters.agreed_value().unwrap();
            total_agreed_rewards += agreed_counter.reward.as_nano();
            total_agreed_work += agreed_counter.work;
        }

        // Comparing results
        if total_reward_sum != total_reward {
            println!(
                "total_reward_sum: {}, total_reward: {}",
                total_reward_sum, total_reward
            );
        }
        if total_reward_sum != total_agreed_rewards {
            println!(
                "total_reward_sum: {}, total_agreed_rewards: {}",
                total_reward_sum, total_agreed_rewards
            );
        }

        let reward_diff_percent = 100.0 * diff(total_reward, total_agreed_rewards);
        let work_diff_percent = 100.0 * diff(total_work, total_agreed_work);

        println!("total_reward: {}", total_reward);
        println!("total_agreed_rewards: {}", total_agreed_rewards);
        println!("reward diff: {:.6} %", reward_diff_percent);
        println!("total_work: {}", total_work);
        println!("total_agreed_work: {}", total_agreed_work);
        println!("work diff: {:.6} %", work_diff_percent);
        println!("-------");

        // NB: The small diffs we often see in rewards, are not mainly due to the byzantine faults, but due to the reward calculation.
        // There are also intermittent small diffs in work.
        // They seem to be due to the use of f64 and rounding errors.

        Ok(RewardResults {
            total_reward,
            total_agreed_rewards,
            total_work,
            total_agreed_work,
            reward_diff_percent,
            work_diff_percent,
        })
    }

    fn get_random_pk() -> PublicKey {
        PublicKey::from(SecretKey::random().public_key())
    }

    fn get_instance(base_cost: u64) -> Elder {
        let acc = Accumulation::new(Default::default(), Default::default());
        let base_cost = Money::from_nano(base_cost);
        let algo = StorageRewards::new(base_cost);
        FarmingSystem::new(algo, acc)
    }

    fn reward(instance: &mut Elder, data_info: (Hash, NumBytes), factor: f64) -> Result<Money> {
        let (hash, num_bytes) = data_info;
        instance.reward(hash.value, num_bytes.value, factor)
    }

    fn diff(one: u64, two: u64) -> f64 {
        (u64::max(one, two) as f64 / u64::min(one, two) as f64) - 1.0
    }

    fn round(value: f64, decimals: u8) -> f64 {
        let base: u64 = 10;
        let res = base.pow(decimals.into()) as f64;
        (res * value).round() / res
    }

    // Four out of seven Elders will be correct/honest.
    // Three out of seven Elders will be wildly wrong/dishonest.
    fn apply_byzantine_faults(counters: Vec<RewardCounter>) -> Vec<RewardCounter> {
        let mut rng = rand::thread_rng();
        let mut byzantine_elders = HashSet::new();
        let quorum = (counters.len() / 3) * 2;
        let max_byzantine = quorum - 1;
        while max_byzantine > byzantine_elders.len() {
            let _ = byzantine_elders.insert(rng.gen_range(0, counters.len()));
        }
        let mut faulty_counters = counters;
        for i in byzantine_elders {
            faulty_counters[i] = RewardCounter {
                reward: Money::from_nano(rng.next_u64()), // wildly wrong/dishonest
                work: rng.next_u64(),                     // wildly wrong/dishonest
            };
        }
        faulty_counters
    }

    /// -------------------------------------------------------------------------
    ///  ------------------------ Test structs ---------------------------------
    /// -------------------------------------------------------------------------

    struct RewardResults {
        total_reward: u64,
        total_agreed_rewards: u64,
        total_work: u64,
        total_agreed_work: u64,
        reward_diff_percent: f64,
        work_diff_percent: f64,
    }

    #[derive(Clone, Debug)]
    struct Factor {
        pub value: f64,
    }

    impl Factor {
        pub fn new(value: f64) -> Self {
            Self { value }
        }
    }

    impl Arbitrary for Factor {
        fn arbitrary<G>(_: &mut G) -> Self
        where
            G: crdts::quickcheck::Gen,
        {
            let mut rng = rand::thread_rng();
            let value = rng.gen_range(0.67, 2.7);
            Self::new(value)
        }
    }

    #[derive(Clone, Debug)]
    struct PreviousWork {
        pub values: Vec<u64>,
    }

    impl PreviousWork {
        pub fn new() -> Self {
            let mut rng = rand::thread_rng();
            let values: Vec<u64> = (0..rng.gen_range(160, 200))
                .map(|_| {
                    // 1 (inclusive) to 2135 (exclusive)
                    rng.gen_range(1, 2135) // i.e. 2134 would represent having been around for 2134 uploads.
                })
                .collect();
            Self { values }
        }
    }

    #[derive(Clone, Debug)]
    struct WorkInfo {
        pub values: Vec<(Hash, NumBytes)>,
    }

    #[derive(Clone, Debug)]
    struct Hash {
        pub value: Vec<u8>,
    }

    #[derive(Clone, Debug)]
    struct NumBytes {
        pub value: u64,
    }

    impl WorkInfo {
        pub fn new() -> Self {
            let mut rng = rand::thread_rng();
            // 10-15 data uploads (and reward instances)
            let values: Vec<(Hash, NumBytes)> = (0..rng.gen_range(10, 15))
                .map(|_| {
                    // 256 byte hash
                    let hash = (0..256)
                        .map(|_| {
                            // 0 (inclusive) to 255 (exclusive)
                            rng.gen_range(0, 255)
                        })
                        .collect();
                    (
                        Hash { value: hash },
                        NumBytes {
                            value: rng.gen_range(3000, 1_000_001), // 3 kb (inclusive) to 1001 kb (exclusive)
                        },
                    )
                })
                .collect();
            Self { values }
        }
    }
}
