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
    ///
    pub fn new(farming_algo: A, accumulation: Accumulation) -> Self {
        Self {
            farming_algo,
            accumulation,
        }
    }

    /// The work counter
    pub fn add_account(&mut self, id: AccountId, worked: WorkCounter) -> Result<()> {
        let e = self.accumulation.add_account(id, worked)?;
        Ok(self.accumulation.apply(AccumulationEvent::AccountAdded(e)))
    }

    /// Factor is a number > 0, by which reward will be increased or decreased.
    ///
    /// Temp comments:
    ///
    /// When factor is > 1, the StoreCost is topped up with the surplus
    /// from section account, to form the total reward.
    /// This is essentially the same as a _net farming_, aka net issuance of money.
    ///
    /// When factor is < 1, the excess from the StoreCost
    /// stays at the section account.
    /// This is essentially the same as recycling money, moving it out of circulation.
    pub fn reward(&mut self, data_hash: Vec<u8>, num_bytes: usize, factor: f64) -> Result<()> {
        let accounts_work: HashMap<AccountId, WorkCounter> = self
            .accumulation
            .get_all()
            .into_iter()
            .map(|(id, acc)| (*id, acc.worked))
            .collect();
        let work_cost = self.farming_algo.work_cost(num_bytes);
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

mod test {
    use super::{Accumulation, FarmingSystem};
    use crate::calculation::BasicRewards;
    use safe_nd::{AccountId, Error, Money, PublicKey, Result};
    use threshold_crypto::SecretKey;

    macro_rules! hashmap {
        ($( $key: expr => $val: expr ),*) => {{
             let mut map = ::std::collections::HashMap::new();
             $( let _ = map.insert($key, $val); )*
             map
        }}
    }

    #[test]
    fn farming_system() -> Result<()> {
        // --- Arrange ---
        let acc = Accumulation::new(Default::default(), Default::default());
        let base_cost = Money::from_nano(2);
        let algo = BasicRewards::new(base_cost);
        let mut system = FarmingSystem::new(algo, acc);

        let account = get_random_pk();
        let data_hash = vec![1, 2, 3];

        let num_bytes = 3;
        let factor = 2.0; // reward will be 2x StoreCost, where half is contributed by the network.
        let worked = 1;

        // --- Act ---
        // Try accumulate.
        let _ = system.add_account(account, worked)?;
        let _ = system.reward(data_hash, num_bytes, factor)?;

        // --- Assert ---
        match system.claim(account) {
            Err(_) => assert!(false),
            Ok(e) => {
                // println!("Amount: {}", e.amount.as_nano());
                // println!("Worked: {}", e.worked);
                assert!(
                    e.amount.as_nano() == factor as u64 * (num_bytes as u64 + base_cost.as_nano())
                );
                assert!(e.worked == worked + 1);
            }
        }
        Ok(())
    }

    fn get_random_pk() -> PublicKey {
        PublicKey::from(SecretKey::random().public_key())
    }
}
