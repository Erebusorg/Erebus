use thiserror::Error;

#[derive(Debug, Error)]
pub enum FeeError {
    #[error("a note is 64 bytes of hex: a nullifier and a secret, both reduced")]
    MalformedNote,
    #[error("that commitment is not in the tree, so there is nothing to spend")]
    NoteNotFunded,
    #[error("the payout has to name at least one recipient")]
    EmptyPayout,
    #[error("recipients and amounts have to line up: {recipients} against {amounts}")]
    PayoutMismatch { recipients: usize, amounts: usize },
    #[error("an address is 20 bytes of hex")]
    MalformedAddress,
    #[error("proving failed: {0}")]
    Proving(String),
    #[error("the proof does not verify against these public inputs")]
    Rejected,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
