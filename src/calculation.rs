// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::WorkCounter;
use rand::distributions::{uniform::Uniform as Range, Distribution};
use safe_nd::{AccountId, Money};
use std::collections::HashMap;
use std::hash::Hash;

pub trait RewardAlgo {
    fn set(&mut self, base_cost: Money);
    fn work_cost(&self, work_units: usize) -> Money;
    fn total_reward(&self, factor: f64, work_cost: Money) -> Money;
    fn distribute(
        &self,
        total_reward: Money,
        accounts_work: HashMap<AccountId, WorkCounter>,
    ) -> HashMap<AccountId, Money>;
}

/// Cost of, and rewards for, work.
pub struct BasicRewards {
    base_cost: Money,
}

impl RewardAlgo for BasicRewards {
    /// Use this to update the cost,
    /// as per any desired formula and frequency.
    fn set(&mut self, base_cost: Money) {
        self.base_cost = base_cost;
    }

    /// Work units can for example be
    /// number of bytes to store.
    fn work_cost(&self, work_units: usize) -> Money {
        // 1 nano per work unit + base cost
        Money::from_nano(work_units as u64 + self.base_cost.as_nano())
    }

    fn total_reward(&self, factor: f64, work_cost: Money) -> Money {
        let amount = factor * work_cost.as_nano() as f64;
        Money::from_nano(amount as u64)
    }

    fn distribute(
        &self,
        total_reward: Money,
        accounts_work: HashMap<AccountId, WorkCounter>,
    ) -> HashMap<AccountId, Money> {
        //
        let total_reward = total_reward.as_nano();
        let all_work: WorkCounter = accounts_work.values().sum();

        let mut shares_sum = 0;
        let mut shares: HashMap<AccountId, Money> = Default::default();

        for (id, work) in &accounts_work {
            let share = total_reward / (all_work / work);
            let _ = shares.insert(*id, Money::from_nano(share));
            shares_sum += share;
        }

        // covers probabilistic distribution as well
        if total_reward > shares_sum {
            if let Some(id) = random_key(&accounts_work) {
                if let Some(share) = shares.get(&id) {
                    let remainder = total_reward - shares_sum;
                    let new_share = share.as_nano() + remainder;
                    let _ = shares.insert(*id, Money::from_nano(new_share));
                }
            }
        }

        shares
    }
}

fn random_key<K: Eq + Hash, V>(hash: &HashMap<K, V>) -> Option<&K> {
    if hash.is_empty() {
        return None;
    }
    let index = Range::new(0, hash.len()).sample(&mut rand::thread_rng());
    hash.keys().skip(index).next()
}
