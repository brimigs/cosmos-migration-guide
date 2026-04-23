/**
 * Merkle Tree Generator for Token Claims
 *
 * Reads a snapshot JSON file and generates:
 * 1. Merkle root (to be stored on-chain)
 * 2. Individual proofs for each address (to be served to users)
 *
 * Usage: npx ts-node generate-merkle-tree.ts <snapshot.json> <output.json>
 */

import * as anchor from "@anchor-lang/core";
import * as fs from "fs";
import { PublicKey } from "@solana/web3.js";
import { HashingAlgorithm, MerkleTree } from "svm-merkle-tree";

interface SnapshotEntry {
  cosmos_address: string;
  solana_address: string;
  amount: string | number;
}

interface Snapshot {
  snapshot_height: number;
  chain_id: string;
  timestamp: string;
  entries: SnapshotEntry[];
}

interface ProofEntry {
  solana_address: string;
  cosmos_address: string;
  amount: string;
  index: number;
  proof: string;
}

interface Output {
  merkle_root: number[];
  merkle_root_hex: string;
  total_amount: string;
  total_entries: number;
  snapshot_height: number;
  proofs: ProofEntry[];
}

function parseAmount(amount: string | number): anchor.BN {
  if (typeof amount === "number") {
    if (!Number.isSafeInteger(amount) || amount < 0) {
      throw new Error(`invalid numeric amount: ${amount}`);
    }
    return new anchor.BN(amount);
  }

  if (!/^\d+$/.test(amount)) {
    throw new Error(`invalid string amount: ${amount}`);
  }

  return new anchor.BN(amount, 10);
}

function createLeafBytes(
  solanaAddress: PublicKey,
  amount: anchor.BN
): Buffer {
  return Buffer.concat([
    solanaAddress.toBuffer(),
    Buffer.from(new Uint8Array(amount.toArray("le", 8))),
  ]);
}

function generateMerkleTree(snapshotPath: string, outputPath: string): void {
  const snapshotData = fs.readFileSync(snapshotPath, "utf-8");
  const snapshot: Snapshot = JSON.parse(snapshotData);

  console.log(`Processing snapshot from ${snapshot.chain_id}`);
  console.log(`Snapshot height: ${snapshot.snapshot_height}`);
  console.log(`Total entries: ${snapshot.entries.length}`);

  const merkleTree = new MerkleTree(HashingAlgorithm.Sha256, 32);
  const validEntries: Array<SnapshotEntry & { amountBn: anchor.BN }> = [];

  for (const entry of snapshot.entries) {
    try {
      const solanaKey = new PublicKey(entry.solana_address);
      const amountBn = parseAmount(entry.amount);
      const leafBytes = createLeafBytes(solanaKey, amountBn);
      merkleTree.add_leaf(leafBytes);
      validEntries.push({ ...entry, amountBn });
    } catch (error) {
      console.warn(
        `Skipping invalid entry for ${entry.cosmos_address}: ${
          error instanceof Error ? error.message : String(error)
        }`
      );
    }
  }

  merkleTree.merklize();
  const merkleRoot = Array.from(merkleTree.get_merkle_root());
  const merkleRootHex = Buffer.from(merkleRoot).toString("hex");

  console.log(`\nMerkle root: 0x${merkleRootHex}`);

  const proofs: ProofEntry[] = validEntries.map((entry, index) => {
    const proof = merkleTree.merkle_proof_index(index);
    const proofBytes = Buffer.from(proof.get_pairing_hashes());

    return {
      solana_address: entry.solana_address,
      cosmos_address: entry.cosmos_address,
      amount: entry.amountBn.toString(),
      index,
      proof: proofBytes.toString("hex"),
    };
  });

  const totalAmount = validEntries.reduce(
    (sum, entry) => sum.add(entry.amountBn),
    new anchor.BN(0)
  );

  const output: Output = {
    merkle_root: merkleRoot,
    merkle_root_hex: merkleRootHex,
    total_amount: totalAmount.toString(),
    total_entries: validEntries.length,
    snapshot_height: snapshot.snapshot_height,
    proofs,
  };

  fs.writeFileSync(outputPath, JSON.stringify(output, null, 2));
  console.log(`\nOutput written to: ${outputPath}`);
  console.log(`Total claimable amount: ${output.total_amount}`);
}

const args = process.argv.slice(2);
if (args.length < 2) {
  console.log("Usage: npx ts-node generate-merkle-tree.ts <snapshot.json> <output.json>");
  console.log("\nExample:");
  console.log("  npx ts-node generate-merkle-tree.ts sample-snapshot.json merkle-output.json");
  process.exit(1);
}

generateMerkleTree(args[0], args[1]);
