import fc from 'fast-check';
import type { Model, Real } from './types';
import {
  getWalletNameByAddress,
  isStakerActive,
  logCommand,
  refreshModel,
  trackCommandRun,
} from './utils';
import { expect } from 'vitest';
import { txErr } from '@clarigen/test';
import { randomPoxAddress } from '../../test-helpers';
import { errorCodes } from '../pox-5-helpers';

export const StakeErr = (accounts: Real['accounts']) =>
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
      check: (model: Readonly<Model>) => isStakerActive(model, r.sender),
      run: (model: Model, real: Real) => {
        refreshModel(model, real);
        trackCommandRun(model, 'stake_err');

        const bitcoinHeightBefore = real.network.burnBlockHeight;
        const stacksHeightBefore = real.network.stacksBlockHeight;

        const receipt = txErr(
          real.contracts.pox5.stake({
            amountUstx: r.amountUstx,
            poxAddr: randomPoxAddress(r.poxAddrSeed),
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

        expect(receipt.value).toBe(errorCodes.ERR_ALREADY_STAKED);

        logCommand({
          sender: getWalletNameByAddress(r.sender),
          action: 'stake-err',
          error: 'ERR_ALREADY_STAKED',
          bitcoinHeightBefore,
          stacksHeightBefore,
        });
      },
      toString: () =>
        `stake-err(${getWalletNameByAddress(r.sender)}, already staking)`,
    }));
