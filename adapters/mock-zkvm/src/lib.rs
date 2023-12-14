#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

use std::io::Write;
use std::sync::{Arc, Condvar, Mutex};

use anyhow::ensure;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use sov_rollup_interface::zk::Matches;

/// A mock commitment to a particular zkVM program.
#[derive(Debug, Clone, PartialEq, Eq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct MockCodeCommitment(pub [u8; 32]);

impl Matches<MockCodeCommitment> for MockCodeCommitment {
    fn matches(&self, other: &MockCodeCommitment) -> bool {
        self.0 == other.0
    }
}

/// A mock proof generated by a zkVM.
#[derive(Debug, Clone, PartialEq, Eq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct MockProof<'a> {
    /// The ID of the program this proof might be valid for.
    pub program_id: MockCodeCommitment,
    /// Whether the proof is valid.
    pub is_valid: bool,
    /// The tamper-proof outputs of the proof.
    pub log: &'a [u8],
}

impl<'a> MockProof<'a> {
    /// Serializes a proof into a writer.
    pub fn encode(&self, mut writer: impl Write) {
        writer.write_all(&self.program_id.0).unwrap();
        let is_valid_byte = if self.is_valid { 1 } else { 0 };
        writer.write_all(&[is_valid_byte]).unwrap();
        writer.write_all(self.log).unwrap();
    }

    /// Serializes a proof into a vector.
    pub fn encode_to_vec(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        encoded
    }

    /// Tries to deserialize a proof from a byte slice.
    pub fn decode(input: &'a [u8]) -> Result<Self, anyhow::Error> {
        ensure!(input.len() >= 33, "Input is too short");
        let program_id = MockCodeCommitment(input[0..32].try_into().unwrap());
        let is_valid = input[32] == 1;
        let log = &input[33..];
        Ok(Self {
            program_id,
            is_valid,
            log,
        })
    }
}

#[derive(Clone)]
struct Notifier {
    notified: Arc<Mutex<bool>>,
    cond: Arc<Condvar>,
}

impl Default for Notifier {
    fn default() -> Self {
        Self {
            notified: Arc::new(Mutex::new(false)),
            cond: Default::default(),
        }
    }
}

impl Notifier {
    fn wait(&self) {
        let mut notified = self.notified.lock().unwrap();
        while !*notified {
            notified = self.cond.wait(notified).unwrap();
        }
    }

    fn notify(&self) {
        let mut notified = self.notified.lock().unwrap();
        *notified = true;
        self.cond.notify_all();
    }
}

/// A mock implementing the zkVM trait.
#[derive(Clone, Default)]
pub struct MockZkvm {
    worker_thread_notifier: Notifier,
}

impl MockZkvm {
    /// Simulates zk proof generation.
    pub fn make_proof(&self) {
        // We notify the worket thread.
        self.worker_thread_notifier.notify();
    }
}

impl sov_rollup_interface::zk::Zkvm for MockZkvm {
    type CodeCommitment = MockCodeCommitment;

    type Error = anyhow::Error;

    fn verify<'a>(
        serialized_proof: &'a [u8],
        code_commitment: &Self::CodeCommitment,
    ) -> Result<&'a [u8], Self::Error> {
        let proof = MockProof::decode(serialized_proof)?;
        anyhow::ensure!(
            proof.program_id.matches(code_commitment),
            "Proof failed to verify against requested code commitment"
        );
        anyhow::ensure!(proof.is_valid, "Proof is not valid");
        Ok(proof.log)
    }

    fn verify_and_extract_output<
        Add: sov_rollup_interface::RollupAddress,
        Da: sov_rollup_interface::da::DaSpec,
        Root: serde::Serialize + serde::de::DeserializeOwned,
    >(
        serialized_proof: &[u8],
        code_commitment: &Self::CodeCommitment,
    ) -> Result<sov_rollup_interface::zk::StateTransition<Da, Add, Root>, Self::Error> {
        let output = Self::verify(serialized_proof, code_commitment)?;
        Ok(bincode::deserialize(output)?)
    }
}

impl sov_rollup_interface::zk::ZkvmHost for MockZkvm {
    type Guest = MockZkGuest;

    fn add_hint<T: Serialize>(&mut self, _item: T) {}

    fn simulate_with_hints(&mut self) -> Self::Guest {
        MockZkGuest {}
    }

    fn run(&mut self, _with_proof: bool) -> Result<sov_rollup_interface::zk::Proof, anyhow::Error> {
        self.worker_thread_notifier.wait();
        Ok(sov_rollup_interface::zk::Proof::Empty)
    }
}

/// A mock implementing the Guest.
pub struct MockZkGuest {}

impl sov_rollup_interface::zk::Zkvm for MockZkGuest {
    type CodeCommitment = MockCodeCommitment;

    type Error = anyhow::Error;

    fn verify<'a>(
        _serialized_proof: &'a [u8],
        _code_commitment: &Self::CodeCommitment,
    ) -> Result<&'a [u8], Self::Error> {
        unimplemented!()
    }

    fn verify_and_extract_output<
        Add: sov_rollup_interface::RollupAddress,
        Da: sov_rollup_interface::da::DaSpec,
        Root: Serialize + serde::de::DeserializeOwned,
    >(
        _serialized_proof: &[u8],
        _code_commitment: &Self::CodeCommitment,
    ) -> Result<sov_rollup_interface::zk::StateTransition<Da, Add, Root>, Self::Error> {
        unimplemented!()
    }
}

impl sov_rollup_interface::zk::ZkvmGuest for MockZkGuest {
    fn read_from_host<T: serde::de::DeserializeOwned>(&self) -> T {
        unimplemented!()
    }

    fn commit<T: Serialize>(&self, _item: &T) {
        unimplemented!()
    }
}

#[test]
fn test_mock_proof_round_trip() {
    let proof = MockProof {
        program_id: MockCodeCommitment([1; 32]),
        is_valid: true,
        log: &[2; 50],
    };

    let mut encoded = Vec::new();
    proof.encode(&mut encoded);

    let decoded = MockProof::decode(&encoded).unwrap();
    assert_eq!(proof, decoded);
}