//! Device room participation. Link is the only forum port; Threads owns MLS.
#![forbid(unsafe_code)]
use cgrp::{Effect, Level, PolicySource, PreparedEffect};
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

pub type Result<T> = std::result::Result<T, Error>;
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + 'a>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error, Serialize, schemars::JsonSchema)]
pub enum Error {
    #[error("invalid participation transition")]
    Invalid,
    #[error("current room authority unavailable")]
    Authority,
    #[error("MLS or Link integration unavailable")]
    Unavailable,
    #[error("ordered room state needs reconciliation")]
    Reconcile,
    #[error("device checkpoint failed")]
    Storage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Phase {
    Outside,
    ConsentPending,
    Joining,
    AwaitingOrderedCommit,
    Joined,
    Reconciling,
    Withdrawn,
}

#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrderRequest {
    pub room: [u8; 16],
    /// Assembly's volatile ordering generation, distinct from the MLS epoch.
    pub ordering: [u8; 16],
    pub expected: u64,
    pub encrypted_commit: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrderReply {
    pub ordering: [u8; 16],
    pub counter: u64,
    pub proof: Vec<u8>,
}

/// Only the sole clnk connection implements this transport at composition.
pub trait Link {
    fn order<'a>(&'a mut self, request: &'a OrderRequest) -> PortFuture<'a, OrderReply>;
}

/// No public constructors: real MLS and ordered evidence must first be adapted
/// and independently reviewed at the Threads integration boundary.
pub struct PreparedJoin {
    ordering: [u8; 16],
    expected_counter: u64,
    encrypted_commit: Vec<u8>,
    epoch: u64,
}
pub struct OrderedJoin {
    ordering: [u8; 16],
    counter: u64,
    epoch: u64,
    encrypted_handover: Vec<Vec<u8>>,
}

pub trait Threads {
    fn prepare<'a>(
        &'a mut self,
        group: &'a [u8; 32],
        membership_revision: u64,
    ) -> PortFuture<'a, PreparedJoin>;
    fn verify_order<'a>(
        &'a mut self,
        request: &'a OrderRequest,
        reply: &'a OrderReply,
    ) -> PortFuture<'a, OrderedJoin>;
}

pub struct UnavailableThreads;
impl Threads for UnavailableThreads {
    fn prepare<'a>(&'a mut self, _: &'a [u8; 32], _: u64) -> PortFuture<'a, PreparedJoin> {
        Box::pin(async { Err(Error::Unavailable) })
    }
    fn verify_order<'a>(
        &'a mut self,
        _: &'a OrderRequest,
        _: &'a OrderReply,
    ) -> PortFuture<'a, OrderedJoin> {
        Box::pin(async { Err(Error::Unavailable) })
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    phase: Phase,
    community: String,
    room: [u8; 16],
    group: [u8; 32],
    policy_epoch: u64,
    preparation_valid_until: u64,
    membership_revision: u64,
    local_revision: u64,
    epoch: u64,
    ordering: [u8; 16],
    counter: u64,
    pending: Option<OrderRequest>,
}

/// Combined cwst transaction with the owning Groups/Threads candidate and
/// encrypted handover outbox. Unknown outcomes must return Storage and reload.
/// This object represents that already-prepared shared transaction, not a
/// standalone Attend write. It must fence every participating owner revision.
pub trait Persistence {
    fn commit<'a>(
        &'a mut self,
        expected: u64,
        checkpoint: &'a Checkpoint,
        encrypted_outputs: &'a [Vec<u8>],
    ) -> PortFuture<'a, ()>;
}

pub struct Attend {
    checkpoint: Checkpoint,
}
impl Attend {
    pub fn new() -> Self {
        Self {
            checkpoint: Checkpoint {
                phase: Phase::Outside,
                community: String::new(),
                room: [0; 16],
                group: [0; 32],
                policy_epoch: 0,
                preparation_valid_until: 0,
                membership_revision: 0,
                local_revision: 0,
                epoch: 0,
                ordering: [0; 16],
                counter: 0,
                pending: None,
            },
        }
    }
    pub fn phase(&self) -> &Phase {
        &self.checkpoint.phase
    }

    /// A genuine Groups preparation is necessary but is not a committed join.
    pub fn begin(
        &mut self,
        decision: &PreparedEffect<'_>,
        member: &str,
        room: [u8; 16],
        source: &impl PolicySource,
        now: u64,
    ) -> Result<()> {
        if !matches!(self.checkpoint.phase, Phase::Outside | Phase::Withdrawn) {
            return Err(Error::Invalid);
        }
        let Effect::Membership {
            group,
            members,
            level,
            policy_epoch,
            revision,
            ..
        } = decision.event()
        else {
            return Err(Error::Authority);
        };
        if now >= decision.valid_until()
            || !matches!(level, Level::Opening | Level::Public)
            || !members.contains(member)
        {
            return Err(Error::Authority);
        }
        let policy = source
            .current(decision.community(), now)
            .map_err(|_| Error::Authority)?;
        policy.validate(now).map_err(|_| Error::Authority)?;
        if policy.epoch != *policy_epoch {
            return Err(Error::Authority);
        }
        self.checkpoint.community = decision.community().into();
        self.checkpoint.group = *group;
        self.checkpoint.room = room;
        self.checkpoint.policy_epoch = *policy_epoch;
        self.checkpoint.preparation_valid_until = decision.valid_until();
        self.checkpoint.membership_revision = *revision;
        self.checkpoint.phase = Phase::ConsentPending;
        Ok(())
    }
    fn current(&self, source: &impl PolicySource, now: u64) -> Result<()> {
        if now >= self.checkpoint.preparation_valid_until {
            return Err(Error::Authority);
        }
        let p = source
            .current(&self.checkpoint.community, now)
            .map_err(|_| Error::Authority)?;
        p.validate(now).map_err(|_| Error::Authority)?;
        if p.epoch != self.checkpoint.policy_epoch {
            return Err(Error::Authority);
        }
        Ok(())
    }
    /// Persist the exact proposal before it can reach Link. Retry never asks
    /// Threads to produce a second external commit for the same prepared join.
    pub async fn prepare_join(
        &mut self,
        threads: &mut dyn Threads,
        store: &mut dyn Persistence,
        source: &impl PolicySource,
        now: u64,
    ) -> Result<()> {
        if self.checkpoint.phase != Phase::ConsentPending {
            return Err(Error::Invalid);
        }
        self.current(source, now)?;
        let prepared = threads
            .prepare(&self.checkpoint.group, self.checkpoint.membership_revision)
            .await?;
        if prepared.encrypted_commit.is_empty() || prepared.ordering == [0; 16] {
            return Err(Error::Authority);
        }
        let mut next = self.checkpoint.clone();
        next.epoch = prepared.epoch;
        next.ordering = prepared.ordering;
        next.pending = Some(OrderRequest {
            room: next.room,
            ordering: prepared.ordering,
            expected: prepared.expected_counter,
            encrypted_commit: prepared.encrypted_commit,
        });
        next.phase = Phase::Joining;
        self.persist(next, &[], store).await
    }
    pub async fn order(
        &mut self,
        link: &mut dyn Link,
        source: &impl PolicySource,
        now: u64,
    ) -> Result<OrderReply> {
        if !matches!(
            self.checkpoint.phase,
            Phase::Joining | Phase::AwaitingOrderedCommit
        ) {
            return Err(Error::Invalid);
        }
        self.current(source, now)?;
        let pending = self.checkpoint.pending.as_ref().ok_or(Error::Invalid)?;
        let reply = link.order(pending).await?;
        if reply.ordering != pending.ordering
            || pending.expected.checked_add(1) != Some(reply.counter)
        {
            self.checkpoint.phase = Phase::Reconciling;
            return Err(Error::Reconcile);
        }
        self.checkpoint.phase = Phase::AwaitingOrderedCommit;
        Ok(reply)
    }
    pub async fn accept(
        &mut self,
        reply: &OrderReply,
        threads: &mut dyn Threads,
        store: &mut dyn Persistence,
        source: &impl PolicySource,
        now: u64,
    ) -> Result<Vec<Vec<u8>>> {
        if self.checkpoint.phase != Phase::AwaitingOrderedCommit {
            return Err(Error::Invalid);
        }
        self.current(source, now)?;
        let pending = self.checkpoint.pending.as_ref().ok_or(Error::Invalid)?;
        if reply.ordering != pending.ordering
            || pending.expected.checked_add(1) != Some(reply.counter)
        {
            self.checkpoint.phase = Phase::Reconciling;
            return Err(Error::Reconcile);
        }
        let evidence = threads.verify_order(pending, reply).await?;
        if evidence.ordering != pending.ordering
            || evidence.counter != reply.counter
            || evidence.epoch != self.checkpoint.epoch
        {
            self.checkpoint.phase = Phase::Reconciling;
            return Err(Error::Reconcile);
        }
        let mut next = self.checkpoint.clone();
        next.counter = evidence.counter;
        next.phase = Phase::Joined;
        next.pending = None;
        self.persist(next, &evidence.encrypted_handover, store)
            .await?;
        Ok(evidence.encrypted_handover)
    }
    async fn persist(
        &mut self,
        mut next: Checkpoint,
        outputs: &[Vec<u8>],
        store: &mut dyn Persistence,
    ) -> Result<()> {
        next.local_revision = next.local_revision.checked_add(1).ok_or(Error::Invalid)?;
        // Set reconciliation *before* await: cancellation and unknown storage
        // outcomes cannot leave this handle capable of publishing stale state.
        self.checkpoint.phase = Phase::Reconciling;
        store
            .commit(self.checkpoint.local_revision, &next, outputs)
            .await?;
        self.checkpoint = next;
        Ok(())
    }
    /// An offline admitting peer does not create authority or a different commit.
    pub fn admitting_peer_offline(&mut self) -> Result<()> {
        if !matches!(
            self.checkpoint.phase,
            Phase::Joining | Phase::AwaitingOrderedCommit
        ) {
            return Err(Error::Invalid);
        }
        self.checkpoint.phase = Phase::Joining;
        Ok(())
    }
    pub async fn withdraw(&mut self, store: &mut dyn Persistence) -> Result<()> {
        if self.checkpoint.phase == Phase::Reconciling {
            return Err(Error::Reconcile);
        }
        let mut next = self.checkpoint.clone();
        next.phase = Phase::Withdrawn;
        next.pending = None;
        self.persist(next, &[], store).await
    }
}
impl Default for Attend {
    fn default() -> Self {
        Self::new()
    }
}

/// Filter bounds only; Threads decrypts/imports accepted history and encrypts
/// profile key handover. A Circle never inherits newcomer history.
pub fn history_window(
    level: Level,
    band: &cgrp::Band,
    joined_at: u64,
    timestamps: &[u64],
) -> Result<Vec<usize>> {
    if level == Level::Circle {
        return Ok(vec![]);
    }
    if timestamps.windows(2).any(|w| w[0] > w[1]) {
        return Err(Error::Invalid);
    }
    let mut indices: Vec<usize> = timestamps
        .iter()
        .enumerate()
        .filter(|(_, t)| cgrp::history_allowed(joined_at, **t, band))
        .map(|(i, _)| i)
        .collect();
    let keep = indices
        .len()
        .saturating_sub(usize::from(band.history_messages));
    indices.drain(..keep);
    Ok(indices)
}

#[cfg(test)]
#[path = "../tests/internal/mod.rs"]
mod tests;
