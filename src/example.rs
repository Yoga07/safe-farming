// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    calculation::*, AccountId, Accumulation, AccumulationEvent, CurrentAccumulation, WorkCounter,
};
use safe_nd::Result;
use std::collections::HashMap;

struct FarmingSystem<A: RewardAlgo> {
    farming_algo: A,
    accumulation: Accumulation,
}

#[allow(unused)]
impl<A: RewardAlgo> FarmingSystem<A> {
    /// The work counter
    pub fn add_account(&mut self, id: AccountId, worked: WorkCounter) -> Result<()> {
        let e = self.accumulation.add_account(id, worked)?;
        Ok(self.accumulation.apply(AccumulationEvent::AccountAdded(e)))
    }

    pub fn reward(&mut self, data_hash: Vec<u8>, num_bytes: usize) -> Result<()> {
        let accounts_work: HashMap<AccountId, WorkCounter> = self
            .accumulation
            .get_all()
            .into_iter()
            .map(|(id, acc)| (*id, acc.worked))
            .collect();
        let work_cost = self.farming_algo.work_cost(num_bytes);
        let factor = 2.0; // This means that the section is decreasing its balance with work_cost
        let total_reward = self.farming_algo.total_reward(factor, work_cost);
        let distribution = self.farming_algo.distribute(total_reward, accounts_work);
        let e = self.accumulation.accumulate(data_hash, distribution)?;
        Ok(self
            .accumulation
            .apply(AccumulationEvent::AmountsAccumulated(e)))
    }

    pub fn claim(&mut self, id: AccountId) -> Result<CurrentAccumulation> {
        let e = self.accumulation.claim(id)?;
        self.accumulation
            .apply(AccumulationEvent::AccumulatedClaimed(e.clone()));
        Ok(e.accumulated)
    }
}
