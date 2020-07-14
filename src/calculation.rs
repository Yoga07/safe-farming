// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// http://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

use safe_nd::{AccountId, Money, Work};
use std::{cmp::Ordering, collections::HashMap};

/// This algo allows for setting a base cost together with a
/// cost proportional to some work, as measured by a minimum work unit.

pub trait RewardAlgo {
    /// Set the base cost of work.
    fn set(&mut self, base_cost: Money);
    /// Get the cost of work for the specified number of reward units.
    /// It can be simple, 1 RU == 1 unit of money (+ base cost). Or something else.
    fn work_cost(&self, reward_units: u64) -> Money;
    /// Get the total reward implied by the work cost,
    /// as scaled by a factor representing a function of parameters
    /// relevant to the implementing layer.
    fn total_reward(&self, factor: f64, work_cost: Money) -> Money;
    /// Returns the distribution of the total_reward, between
    /// the accounts supplied, proportionally to their accumulated work.
    fn distribute(
        &self,
        total_reward: Money,
        accounts_work: HashMap<AccountId, Work>,
    ) -> HashMap<AccountId, Money>;
}

/// Cost of, and rewards for, storage.
#[derive(Clone)]
pub struct StorageRewards {
    base_cost: Money,
}

impl StorageRewards {
    /// Passed in is the base cost
    /// for buying a unit of work.
    pub fn new(base_cost: Money) -> Self {
        Self { base_cost }
    }
}

/// _Explanation_
/// A unit of Work is defined as, and registered, based on what ever
/// scheme the implementing layer decides.
///
/// New rewards are paid proportionally to Work performed.
///
/// _Implications of StorageRewards design_
/// `StorageRewards` does not define Work, and thus what
/// increases the participants registered Work.
///
/// It could be that `p.work += rewards[p.id]`
/// it could also be that `p.work += 1` for everytime they get a(ny) reward.
/// But it could actually be anything.
///
/// The current implementation in `accumulation.rs` uses `p.work += 1` for every reward received (any amount).
/// The implications of this, is that no matter how small your reward is,
/// your contribution is recorded as equal to any other. This means, that we are defining a `WorkUnit`
/// as a measurement of `time participating`.
/// In our context, that means that we decide how much the value of you storing
/// `x bytes` is, depending on for how long you have been around doing that - relative to everyone else.
///
/// Receiving some data when new, is not as appreciated as after having been there relatively long.
/// Because, after all, maybe you're just disappearing shortly, and introducing a lot of work to the others to
/// replicate the data you were supposed to hold. Still though, it is rewarded higher to receive _more_ data,
/// than less data, regardless of how new/old you are, which reflects in the total rewards being higher for more data.
/// In other words: even though the newer participant's share is still smaller relatively to the others,
/// it is higher absolutely, compared to if they'd all be receiving less data.
///
/// This is the rationale and reasoning behind `accumulation.rs` defining a `WorkUnit` as having received data (no matter how much).
/// The `WorkUnit` is closely related to `NodeAge`, and is a way to capture the higher level design decision of
/// rewards being proportional to `NodeAge`, while decoupling it from the actual implementation of `NodeAge` (which is low granular).
///
/// Another implementation of `RewardAlgo`, might want to use something else than `NodeAge`, and thus use another way
/// to account for `Work`; another definition of a `WorkUnit`.
///
impl RewardAlgo for StorageRewards {
    /// Use this to update the base cost,
    /// as per any desired formula and frequency.
    fn set(&mut self, base_cost: Money) {
        self.base_cost = base_cost;
    }

    /// Here, reward units are the
    /// number of bytes to store.
    fn work_cost(&self, num_bytes: u64) -> Money {
        // 1 nano + base cost per reward unit.
        Money::from_nano(num_bytes + self.base_cost.as_nano())
    }

    /// Use the factor to scale
    /// the reward per any desired function of parameters
    /// relevant to the implementing layer.
    /// In SAFE Network context, the factor could be the
    /// output of a function of node count, section count, percent filled etc. etc.
    fn total_reward(&self, factor: f64, work_cost: Money) -> Money {
        let amount = factor * work_cost.as_nano() as f64;
        Money::from_nano(amount.round() as u64)
    }

    #[allow(clippy::needless_range_loop)]
    /// Distribute the reward
    /// according to the accumulated work
    /// associated with the ids.
    /// Also returns those who got 0 reward
    /// (when their work or total_reward wasn't high enough).
    fn distribute(
        &self,
        total_reward: Money,
        accounts_work: HashMap<AccountId, Work>,
    ) -> HashMap<AccountId, Money> {
        //
        let total_reward = total_reward.as_nano();
        let all_work: Work = accounts_work.values().sum();

        let mut shares_sum = 0;
        let mut shares: Vec<(AccountId, u64)> = Default::default();

        for (id, work) in &accounts_work {
            let share = (total_reward as f64 / (all_work as f64 / *work as f64)).round() as u64;
            shares.push((*id, share));
            shares_sum += share;
        }

        // Add/remove diff.
        match total_reward.cmp(&shares_sum) {
            Ordering::Greater => {
                // Does not cover probabilistic distribution
                // (i.e. when total_reward < number of accounts),
                // since we do not have a shared random value here.
                // We could put it at the acc closest to the data hash though.. TBD
                if !shares.is_empty() {
                    shares.sort_by_key(|t| t.1);
                    let index = 0; // for now, remainder goes to top worker
                    let (id, share) = shares[index];
                    let remainder = total_reward - shares_sum;
                    let new_share = share + remainder;
                    shares[index] = (id, new_share);
                }
            }
            Ordering::Less => {
                let mut diff = shares_sum - total_reward;
                shares.sort_by_key(|t| t.1);
                while diff > 0 {
                    for i in 0..shares.len() {
                        let (id, share) = shares[i];
                        if 0 == diff {
                            break;
                        } else if share >= 1 {
                            shares[i] = (id, share - 1);
                            diff -= 1;
                        }
                    }
                }
            }
            Ordering::Equal => (),
        };

        let shares_sum = (&shares).iter().map(|(_, share)| share).sum();
        if total_reward != shares_sum {
            panic!("total_reward: {}, shares_sum: {}", total_reward, shares_sum);
        }

        shares
            .into_iter()
            .map(|(i, s)| (i, Money::from_nano(s)))
            .collect()
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use safe_nd::{Money, PublicKey, Result};
    use threshold_crypto::SecretKey;

    fn get_random_pk() -> PublicKey {
        PublicKey::from(SecretKey::random().public_key())
    }

    #[test]
    fn distributes_proportionally() -> Result<()> {
        // 7 workers, with accumulated work of 1 to 7, shares 7!=28 nanos of reward.
        // This will result in each worker getting one nano per acc work unit.
        let calc = StorageRewards::new(Money::from_nano(0));
        let accounts_work = (1..8).map(|i| (get_random_pk(), i)).collect();
        let mut dist: Vec<Money> = calc
            .distribute(Money::from_nano(28), accounts_work)
            .into_iter()
            .map(|(_, reward)| reward)
            .collect();
        dist.sort();
        for (i, amount) in dist.iter().enumerate().take(7) {
            assert_eq!(amount.as_nano(), (i + 1) as u64);
        }
        Ok(())
    }
}
