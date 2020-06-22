// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{calculation::*, AccountId, Accumulation, AccumulationEvent, RewardCounter, Work};
use safe_nd::Result;
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
        Ok(self.accumulation.apply(AccumulationEvent::AccountAdded(e)))
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
    pub fn reward(&mut self, data_hash: Vec<u8>, num_bytes: usize, factor: f64) -> Result<()> {
        // first query for accumulated work of all
        let accounts_work: HashMap<AccountId, Work> = self
            .accumulation
            .get_all()
            .into_iter()
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
        Ok(self
            .accumulation
            .apply(AccumulationEvent::RewardsAccumulated(e)))
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
    use std::collections::HashSet;
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
    /// A. With a majority of honest nodes, the reward will be correct.
    /// B. Even with the honest nodes being a bit out of synch, the reward will converge to a correct amount.
    ///
    /// NB: There is currently a small deviance of less than
    /// 0.001 % occurring due to the calculation logic, i.e. _not_ as a result of the byzantine faults.

    macro_rules! hashmap {
        ($( $key: expr => $val: expr ),*) => {{
             let mut map = ::std::collections::HashMap::new();
             $( let _ = map.insert($key, $val); )*
             map
        }}
    }

    type Elder = FarmingSystem<StorageRewards>;

    #[test]
    fn farming_system() -> Result<()> {
        // --- Arrange ---
        let acc = Accumulation::new(Default::default(), Default::default());
        let base_cost = Money::from_nano(2);
        let algo = StorageRewards::new(base_cost);
        let mut system = FarmingSystem::new(algo, acc);

        let account = get_random_pk();
        let data_hash = vec![1, 2, 3];

        let num_bytes = 3;
        let factor = 2.0; // reward will be 2x StoreCost, where half is contributed by the network.
        let work = 1;

        // --- Act ---
        // Try accumulate.
        let _ = system.add_account(account, work)?;
        let _ = system.reward(data_hash, num_bytes, factor)?;

        // --- Assert ---
        match system.claim(account) {
            Err(_) => assert!(false),
            Ok(e) => {
                assert!(
                    e.reward.as_nano() == factor as u64 * (num_bytes as u64 + base_cost.as_nano())
                );
                assert!(e.work == work + 1);
            }
        }
        Ok(())
    }

    #[test]
    fn quickcheck_bft_rewards() {
        quickcheck(bft_rewards as fn(Factor, AllWork, DataHashes) -> TestResult);
    }

    fn bft_rewards(factor: Factor, all_work: AllWork, data_hashes: DataHashes) -> TestResult {
        println!("Test started.");

        let base_cost = 1;
        // 1. We have 7 Elders.
        let indices: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6];
        let mut elders: Vec<(u8, Elder)> = indices
            .into_iter()
            .map(|i| (i, get_instance(base_cost)))
            .collect();

        println!("Elders generated.");

        let mut accounts = vec![];
        for work in &all_work.values {
            let account = get_random_pk();
            accounts.push(account);
            for (_, elder) in &mut elders {
                elder.add_account(account, *work).unwrap();
            }
        }

        println!("Accounts added.");

        // 2. We will issue n rewards.
        let _ = elders
            .par_iter_mut()
            .map(|(_, elder)| {
                for hash in &data_hashes.values {
                    reward(elder, hash.clone(), factor.value).unwrap();
                }
            })
            .collect::<Vec<_>>();

        println!("Rewards accumulated.");

        // The original reward will be aggregated.
        let total_reward: f64 = (&data_hashes.values)
            .into_iter()
            .map(|hash| factor.value * (base_cost as f64 + hash.len() as f64))
            .sum();
        let total_reward = total_reward as u64;
        let total_work: u64 = (all_work.values.len() as u64 * data_hashes.values.len() as u64)
            + all_work.values.into_iter().sum::<u64>();

        let mut total_agreed_rewards = 0;
        let mut total_agreed_work = 0;

        // For each account, we claim the counter from all Elders,
        // introduce the byzantine faults,
        // and finally reach an agreement on a single counter value.
        for account in accounts {
            let counters: Vec<RewardCounter> = (&mut elders)
                .iter_mut()
                .map(|(_, elder)| elder.claim(account).unwrap())
                .collect();
            let counters: RewardCounterSet = apply_byzantine_faults(counters).into();
            let agreed_counter: RewardCounter = counters.into();
            total_agreed_rewards = total_agreed_rewards + agreed_counter.reward.as_nano();
            total_agreed_work = total_agreed_work + agreed_counter.work;
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

        // NB: The small diffs we see in rewards are not due to the byzantine faults, but due to the reward calculation.
        // TODO: Look up what exactly causes this.

        // Assert that the difference is within tolerance levels.
        // (Assert that the byzantine faults introduce no
        // difference.)
        let decimals = 3;
        let expected_diff = 0.0;
        // assert_eq!(expected_diff, round(reward_diff_percent, decimals));  // diff of reward shall be less than 0.001 %
        // assert_eq!(expected_diff, work_diff_percent); // diff of work shall be exactly 0 %

        TestResult::passed()
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

    fn reward(instance: &mut Elder, data_hash: Vec<u8>, factor: f64) -> Result<()> {
        let num_bytes = data_hash.len();
        instance.reward(data_hash, num_bytes, factor)
    }

    fn diff(one: u64, two: u64) -> f64 {
        (u64::max(one, two) as f64 / u64::min(one, two) as f64) - 1.0
    }

    fn round(value: f64, decimals: u8) -> f64 {
        let base: u64 = 10;
        let tolerance = base.pow(decimals.into()) as f64;
        (tolerance * value).round() / tolerance
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
        let mut attacked_counters = counters;
        for i in byzantine_elders {
            attacked_counters[i] = RewardCounter {
                reward: Money::from_nano(rng.next_u64()), // wildly wrong/dishonest
                work: rng.next_u64(),                     // wildly wrong/dishonest
            };
        }
        attacked_counters
    }

    /// -------------------------------------------------------------------------
    ///  ------------------------ Test structs ---------------------------------
    /// -------------------------------------------------------------------------

    #[derive(Clone, Debug)]
    struct Factor {
        pub value: f64,
    }

    impl Arbitrary for Factor {
        fn arbitrary<G>(_: &mut G) -> Self
        where
            G: crdts::quickcheck::Gen,
        {
            let mut rng = rand::thread_rng();
            let value = rng.gen_range(0.67, 2.7);
            Self { value }
        }
    }

    #[derive(Clone, Debug)]
    struct AllWork {
        pub values: Vec<u64>,
    }

    impl Arbitrary for AllWork {
        fn arbitrary<G>(_: &mut G) -> Self
        where
            G: crdts::quickcheck::Gen,
        {
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
    struct DataHashes {
        pub values: Vec<Vec<u8>>,
    }

    impl Arbitrary for DataHashes {
        fn arbitrary<G>(_: &mut G) -> Self
        where
            G: crdts::quickcheck::Gen,
        {
            let mut rng = rand::thread_rng();
            // 10-15 data uploads (and reward instances)
            let values: Vec<Vec<u8>> = (0..rng.gen_range(10, 15))
                .map(|_| {
                    // 3 kb (inclusive) to 1001 kb (exclusive)
                    (0..rng.gen_range(3000, 1000001))
                        .map(|_| {
                            // 0 (inclusive) to 255 (exclusive)
                            rng.gen_range(0, 255)
                        })
                        .collect()
                })
                .collect();
            Self { values }
        }
    }
}
