import { describe, test } from 'node:test';
import { PublicKey, Transaction, TransactionInstruction } from '@solana/web3.js';
import { assert } from 'chai';
import { start } from 'solana-bankrun';
import * as borsh from "borsh"
import { utils } from 'ffjavascript';
import * as fs from 'fs';
import * as snarkjs from 'snarkjs';

const { unstringifyBigInts, leInt2Buff, leBuff2int } = utils;

describe('verify-circom-solana', async () => {
  // load program in solana-bankrun
  const PROGRAM_ID = PublicKey.unique();
  const context = await start([{ name: 'verification', programId: PROGRAM_ID }], []);
  const client = context.banksClient;
  const payer = context.payer;

  console.log("Program ID: ", PROGRAM_ID.toBase58());

  test('Test proof', async () => {
    const blockhash = context.lastBlockhash;
    await fs.readFile(`circuit/artifacts/proof.json`, async function (err, fd) {
        if (err) {
          return console.error(err);
        }
        console.log("File opened successfully!");
        var proof = JSON.parse(fd.toString());

        const parsed_proof =  parseProofToBytesArray(JSON.stringify(proof));
        const proofData = {instruction: Instruction.Verify, inputs: [12], proof_a: parsed_proof.proofA, proof_b: parsed_proof.proofB, proof_c: parsed_proof.proofC};
        const verify = new Verify(proofData);

        // generate board proof
        const { _, publicSignals } = await snarkjs.groth16.fullProve(
        {
          a: 3,
          b: 4
        }, 
        `circuit/artifacts/test.wasm`,
        `circuit/artifacts/test_0000.zkey`, 
      );
      console.log("Public signals: ", parseToBytesArray(publicSignals));

        // We set up our instruction first.
        const ix = new TransactionInstruction({
        keys: [{ pubkey: payer.publicKey, isSigner: true, isWritable: true }],
        programId: PROGRAM_ID,
        data: verify.toBuffer(), // No data
        });

        const tx = new Transaction();
        tx.recentBlockhash = blockhash;
        tx.add(ix).sign(payer);

        // Now we process the transaction
        const transaction = await client.processTransaction(tx);
        console.log(transaction.logMessages);
    })
  });
});


// also converts lE to BE
function parseProofToBytesArray(data: any) {
    var mydata = JSON.parse(data.toString());

    for (var i in mydata) {
      if (i == "pi_a" || i == "pi_c") {
        for (var j in mydata[i]) {
          mydata[i][j] = Array.from(
            leInt2Buff(unstringifyBigInts(mydata[i][j]), 32),
          ).reverse();
        }
      } else if (i == "pi_b") {
        for (var j in mydata[i]) {
          for (var z in mydata[i][j]) {
            mydata[i][j][z] = Array.from(
              leInt2Buff(unstringifyBigInts(mydata[i][j][z]), 32),
            );
          }
        }
      }
    }
   
    return {
      proofA: [mydata.pi_a[0], mydata.pi_a[1]].flat(),
      proofB: [
        mydata.pi_b[0].flat().reverse(),
        mydata.pi_b[1].flat().reverse(),
      ].flat(),
      proofC: [mydata.pi_c[0], mydata.pi_c[1]].flat(),
    };
}


function parseToBytesArray(publicSignals: Array<string>) {
      
  var publicInputsBytes = new Array<Array<number>>();
  for (var i in publicSignals) {
    let ref: Array<number> = Array.from([
      ...leInt2Buff(unstringifyBigInts(publicSignals[i]), 32),
    ]).reverse();
    publicInputsBytes.push(ref);
    
  }
  
  return publicInputsBytes
}

// This is a helper class to assign properties to the class
class Assignable {
    constructor(properties) {
      for (const [key, value] of Object.entries(properties)) {
        this[key] = value;
      }
    }
  }
  
  enum Instruction {
    Verify = 0,
  }
  
  class Verify extends Assignable {
  
    number: number;
    instruction: Instruction;
    color: string;
    hobbies: string[];
  
    toBuffer() {
      return Buffer.from(borsh.serialize(VerifySchema, this));
    }
  
    static fromBuffer(buffer: Buffer): Verify {
      return borsh.deserialize({
        struct: { 
          inputs: {
            array: {
                type: "u8"
            }
          },
          proof_a: {
            array: {
                type: "u8"
            }
          },
          proof_b: {
            array: {
                type: "u8"
            }
          },
          proof_c: {
            array: {
                type: "u8"
            }
          }
        }}, buffer) as Verify;
    }
  }
  const VerifySchema = {
    "struct": {
      instruction: "u8",
      inputs: {
        array: {
            type: "u8"
        }
      },
      proof_a: {
        array: {
            type: "u8"
        }
      },
      proof_b: {
        array: {
            type: "u8"
        }
      },
      proof_c: {
        array: {
            type: "u8"
        }
      }
    }
  }
