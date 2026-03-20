import fc from 'fast-check';
import { accounts, project } from '../clarigen-types';
import { projectFactory } from '@clarigen/core';
import { txOk } from '@clarigen/test';
import { test } from 'vitest';

import { MineBitcoinBlocks } from './commands/MineBlocks';
import { Stake } from './commands/Stake';
import { StakeErr } from './commands/StakeErr';
import { Model, Real } from './commands/types';
import { reportCommandRuns } from './commands/utils';
import { initSimnet } from '@stacks/clarinet-sdk';

const contracts = projectFactory(project, 'simnet');

const deployer = accounts.deployer.address;

test('pox-5 stateful property test', async () => {
  const real: Real = {
    accounts,
    contracts,
    network: await initSimnet(),
  };

  // TODO: Adjust parameters for more thorough testing once the test is stable.
  const firstBurnHeight = 0n;
  const prepareCycleLength = 10n;
  const rewardCycleLength = 100n;
  const beginPox5RewardCycle = 0n;

  // Initialize burnchain parameters so stake operations work.
  txOk(
    contracts.pox5.setBurnchainParameters({
      firstBurnHeight: firstBurnHeight,
      prepareCycleLength: prepareCycleLength,
      rewardCycleLength: rewardCycleLength,
      beginPox5RewardCycle: beginPox5RewardCycle,
    }),
    deployer,
  );

  const model: Model = {
    stakers: new Map(),
    burnBlockHeight: BigInt(real.network.burnBlockHeight),
    rewardCycleLength: rewardCycleLength,
    firstBurnHeight: firstBurnHeight,
    prepareCycleLength: prepareCycleLength,
    statistics: new Map(),
  };

  const invariants = [Stake(accounts), StakeErr(accounts), MineBitcoinBlocks()];

  fc.assert(
    fc.property(fc.commands(invariants, { size: 'medium' }), (cmds) => {
      const state = () => ({ model: model, real: real });
      fc.modelRun(state, cmds);
    }),
    { numRuns: 100, verbose: 2 },
  );

  reportCommandRuns(model);
}, 30_000 /* ms timeout */);
