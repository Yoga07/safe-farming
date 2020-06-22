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
use std::hash::Hash;

/// This algo allows for setting a base cost together with a
/// cost proportional to some work, as measured by a minimum work unit.

pub trait RewardAlgo {
    fn set(&mut self, base_cost: Money);
    fn work_cost(&self, work_units: usize) -> Money;
    fn total_reward(&self, factor: f64, work_cost: Money) -> Money;
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
    fn work_cost(&self, num_bytes: usize) -> Money {
        // 1 nano per work unit + base cost
        Money::from_nano(num_bytes as u64 + self.base_cost.as_nano())
    }

    /// Use the factor to scale
    /// the reward per any desired formula.
    fn total_reward(&self, factor: f64, work_cost: Money) -> Money {
        let amount = factor * work_cost.as_nano() as f64;
        Money::from_nano(amount as u64)
    }

    /// Distribute the reward
    /// according to the accumulated work
    /// associated with the ids.
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
            let share = total_reward / (all_work / work);
            shares.push((*id, share));
            shares_sum += share;
        }

        // Add/remove diff.
        if total_reward > shares_sum {
            // covers probabilistic distribution as well
            let index = Range::new(0, shares.len()).sample(&mut rand::thread_rng());
            let (id, share) = shares[index];
            let remainder = total_reward - shares_sum;
            let new_share = share + remainder;
            shares[index] = (id, new_share);
        } else if shares_sum > total_reward {
            let mut diff = shares_sum - total_reward;
            shares.sort_by_key(|t| t.1);
            while diff > 0 {
                for i in 0..shares.len() {
                    let (id, share) = shares[i];
                    if 0 >= diff {
                        break;
                    } else if share > 1 {
                        shares[i] = (id, share - 1);
                        diff = diff - 1;
                    }
                }
            }
        }

        let shares_sum = (&shares).into_iter().map(|(_, share)| share).sum();
        if total_reward != shares_sum {
            println!("total_reward: {}, shares_sum: {}", total_reward, shares_sum);
        }

        shares
            .into_iter()
            .filter(|(_, s)| s > &0)
            .map(|(i, s)| (i, Money::from_nano(s)))
            .collect()
    }
}
