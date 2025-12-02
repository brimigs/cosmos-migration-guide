/**
 * Merkle Tree Generator for Token Claims
 *
 * Reads a snapshot JSON file and generates:
 * 1. Merkle root (to be stored on-chain)
 * 2. Individual proofs for each address (to be served to users)
 *
 * Usage: npx ts-node generate-merkle-tree.ts <snapshot.json> <output.json>
 */

import { PublicKey } from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import { HashingAlgorithm, MerkleTree } from "svm-merkle-tree";
import * as fs from "fs";

interface SnapshotEntry {
  cosmos_address: string;
  solana_address: string;
  amount: number;
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
  amount: number;
  index: number;
  proof: string; // hex-encoded proof bytes
}

interface Output {
  merkle_root: number[]; // 32-byte array for on-chain storage
  merkle_root_hex: string;
  total_amount: number;
  total_entries: number;
  snapshot_height: number;
  proofs: ProofEntry[];
}

function createLeafBytes(
  solanaAddress: PublicKey,
  amount: number,
  isClaimed: boolean
): Buffer {
  // Leaf format: [pubkey (32 bytes) | amount (8 bytes LE) | isClaimed (1 byte)]
  return Buffer.concat([
    solanaAddress.toBuffer(),
    Buffer.from(new Uint8Array(new anchor.BN(amount).toArray("le", 8))),
    Buffer.from([isClaimed ? 1 : 0]),
  ]);
}

function generateMerkleTree(snapshotPath: string, outputPath: string): void {
  // Read snapshot
  const snapshotData = fs.readFileSync(snapshotPath, "utf-8");
  const snapshot: Snapshot = JSON.parse(snapshotData);

  console.log(`Processing snapshot from ${snapshot.chain_id}`);
  console.log(`Snapshot height: ${snapshot.snapshot_height}`);
  console.log(`Total entries: ${snapshot.entries.length}`);

  // Create merkle tree with SHA-256 (native on Solana, more efficient)
  const merkleTree = new MerkleTree(HashingAlgorithm.Sha256, 32);

  // Add all entries as leaves
  const validEntries: SnapshotEntry[] = [];
  for (const entry of snapshot.entries) {
    try {
      const solanaKey = new PublicKey(entry.solana_address);
      const leafBytes = createLeafBytes(solanaKey, entry.amount, false);
      merkleTree.add_leaf(leafBytes);
      validEntries.push(entry);
    } catch (error) {
      console.warn(
        `Skipping invalid Solana address: ${entry.solana_address} (${entry.cosmos_address})`
      );
    }
  }

  // Generate merkle root
  merkleTree.merklize();
  const merkleRoot = Array.from(merkleTree.get_merkle_root());
  const merkleRootHex = Buffer.from(merkleRoot).toString("hex");

  console.log(`\nMerkle root: 0x${merkleRootHex}`);

  // Generate proofs for each entry
  const proofs: ProofEntry[] = validEntries.map((entry, index) => {
    const proof = merkleTree.merkle_proof_index(index);
    const proofBytes = Buffer.from(proof.get_pairing_hashes());

    return {
      solana_address: entry.solana_address,
      cosmos_address: entry.cosmos_address,
      amount: entry.amount,
      index: index,
      proof: proofBytes.toString("hex"),
    };
  });

  // Calculate total
  const totalAmount = validEntries.reduce((sum, e) => sum + e.amount, 0);

  // Build output
  const output: Output = {
    merkle_root: merkleRoot,
    merkle_root_hex: merkleRootHex,
    total_amount: totalAmount,
    total_entries: validEntries.length,
    snapshot_height: snapshot.snapshot_height,
    proofs: proofs,
  };

  // Write output
  fs.writeFileSync(outputPath, JSON.stringify(output, null, 2));
  console.log(`\nOutput written to: ${outputPath}`);
  console.log(`Total claimable amount: ${totalAmount}`);
}

// CLI
const args = process.argv.slice(2);
if (args.length < 2) {
  console.log("Usage: npx ts-node generate-merkle-tree.ts <snapshot.json> <output.json>");
  console.log("\nExample:");
  console.log("  npx ts-node generate-merkle-tree.ts sample-snapshot.json merkle-output.json");
  process.exit(1);
}

generateMerkleTree(args[0], args[1]);
