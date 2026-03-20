import fc from 'fast-check';
import type { Model, Real } from './types';
import {
  currentRewardCycle,
  getWalletNameByAddress,
  isStakerActive,
  logCommand,
  refreshModel,
  rewardCycleToBurnHeight,
  trackCommandRun,
} from './utils';
import { txOk } from '@clarigen/test';
import { expect } from 'vitest';
import { randomPoxAddress } from '../../test-helpers';

export const Stake = (accounts: Real['accounts']) =>
  fc
    .record({
      sender: fc.constantFrom(...Object.values(accounts).map((x) => x.address)),
      amountUstx: fc.bigInt({ min: 1000000n, max: 10000000n }),
      numCycles: fc.integer({ min: 1, max: 12 }),
      poxAddrSeed: fc.uint8Array(),
      signerKey: fc.uint8Array({ maxLength: 33 }),
      signerSig: fc.uint8Array({ maxLength: 65 }),
      unlockBytes: fc.uint8Array({ maxLength: 255 }),
      authId: fc.nat(),
    })
    .map((r) => ({
      check: (model: Readonly<Model>) => !isStakerActive(model, r.sender),
      run: (model: Model, real: Real) => {
        refreshModel(model, real);
        trackCommandRun(model, 'stake');

        const poxAddr = randomPoxAddress(r.poxAddrSeed);

        const bitcoinHeightBefore = real.network.burnBlockHeight;
        const stacksHeightBefore = real.network.stacksBlockHeight;

        const expectedFirstStakedRewardCycle = currentRewardCycle(model) + 1n;
        const expectedUnlockCycle =
          expectedFirstStakedRewardCycle + BigInt(r.numCycles) - 1n;
        const expectedUnlockBurnHeight =
          rewardCycleToBurnHeight(model, expectedUnlockCycle) +
          model.rewardCycleLength / 2n;

        const receipt = txOk(
          real.contracts.pox5.stake({
            amountUstx: r.amountUstx,
            poxAddr,
            signerKey: r.signerKey,
            maxAmount: r.amountUstx,
            authId: r.authId,
            signerSig: r.signerSig,
            startBurnHt: real.network.burnBlockHeight,
            numCycles: r.numCycles,
            unlockBytes: r.unlockBytes,
          }),
          r.sender,
        );

        expect(receipt.value.unlockCycle).toBe(expectedUnlockCycle);
        expect(receipt.value.unlockBurnHeight).toBe(expectedUnlockBurnHeight);

        model.stakers.set(r.sender, {
          amountUstx: r.amountUstx,
          firstRewardCycle: expectedFirstStakedRewardCycle,
          numCycles: BigInt(r.numCycles),
          unlockBurnHeight: expectedUnlockBurnHeight,
          unlockCycle: expectedUnlockCycle,
          isPooled: false,
          poolOwner: null,
        });

        logCommand({
          sender: getWalletNameByAddress(r.sender),
          action: 'stake',
          value: `amount ${r.amountUstx} cycles ${r.numCycles}`,
          bitcoinHeightBefore,
          stacksHeightBefore,
        });
      },
      toString: () =>
        `stake(${getWalletNameByAddress(r.sender)}, ${r.amountUstx}, ${r.numCycles})`,
    }));
