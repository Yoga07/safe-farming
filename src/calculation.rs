// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Work;
use rand::distributions::{uniform::Uniform as Range, Distribution};
use safe_nd::{AccountId, Money};
use std::collections::HashMap;

/// This algo allows for setting a base cost together with a
/// cost proportional to some work, as measured by a minimum work unit.

pub trait RewardAlgo {
    /// Set the base cost of work.
    fn set(&mut self, base_cost: Money);
    /// Get the cost of work for the specified number of units.
    fn work_cost(&self, work_units: u64) -> Money;
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
    ///
    pub fn new(base_cost: Money) -> Self {
        Self { base_cost }
    }
}

impl RewardAlgo for StorageRewards {
    /// Use this to update the base cost,
    /// as per any desired formula and frequency.
    fn set(&mut self, base_cost: Money) {
        self.base_cost = base_cost;
    }

    /// Work units can for example be
    /// number of bytes to store.
    fn work_cost(&self, num_bytes: u64) -> Money {
        // 1 nano per work unit + base cost
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
        if total_reward > shares_sum {
            // Does not cover probabilistic distribution
            // (i.e. when total_reward < number of accounts),
            // since we do not have a shared random value here.
            // We could put it at the acc closest to the data hash though.. TBD
            if shares.len() > 0 {
                shares.sort_by_key(|t| t.1);
                let index = 0; // for now, remainder goes to top worker
                let (id, share) = shares[index];
                let remainder = total_reward - shares_sum;
                let new_share = share + remainder;
                shares[index] = (id, new_share);
            }
        } else if shares_sum > total_reward {
            let mut diff = shares_sum - total_reward;
            shares.sort_by_key(|t| t.1);
            while diff > 0 {
                for i in 0..shares.len() {
                    let (id, share) = shares[i];
                    if 0 >= diff {
                        break;
                    } else if share >= 1 {
                        shares[i] = (id, share - 1);
                        diff = diff - 1;
                    }
                }
            }
        }

        let shares_sum = (&shares).into_iter().map(|(_, share)| share).sum();
        if total_reward != shares_sum {
            panic!("total_reward: {}, shares_sum: {}", total_reward, shares_sum);
        }

        shares
            .into_iter()
            .map(|(i, s)| (i, Money::from_nano(s)))
            .collect()
    }
}

mod test {
    use super::*;
    use safe_nd::{Money, PublicKey, Result};
    use std::collections::{HashMap, HashSet};
    use threshold_crypto::SecretKey;

    macro_rules! hashmap {
        ($( $key: expr => $val: expr ),*) => {{
             let mut map = HashMap::new();
             $( let _ = map.insert($key, $val); )*
             map
        }}
    }

    fn get_random_pk() -> PublicKey {
        PublicKey::from(SecretKey::random().public_key())
    }

    #[test]
    fn distributes_proportionally() -> Result<()> {
        // 7 workers, with accumulated work of 1 to 7, shares 7!=28 nanos of reward.
        // This will result in each worker getting one nano per acc work unit.
        let calc = StorageRewards::new(Money::from_nano(0));
        let accounts_work = (1..8).into_iter().map(|i| (get_random_pk(), i)).collect();
        let mut dist: Vec<Money> = calc
            .distribute(Money::from_nano(28), accounts_work)
            .into_iter()
            .map(|(_, reward)| reward)
            .collect();
        dist.sort();
        for i in 0..7 {
            assert_eq!(dist[i].as_nano(), (i + 1) as u64);
        }
        Ok(())
    }
}
