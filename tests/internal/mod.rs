use super::*;
#[path = "../support/mod.rs"]
mod support;
use cwst::backend::Backend;
use support::{SignedPolicy, TestAuthority, members, policy, request};

macro_rules! vector {
    ($name:ident,$body:block)=>{
        #[cfg(not(target_arch="wasm32"))] #[test] fn $name(){futures::executor::block_on(async $body);}
        #[cfg(target_arch="wasm32")] #[wasm_bindgen_test::wasm_bindgen_test] async fn $name() $body
    };
}
// A public room opens from an ingroup (Fork Open); a private group is a circle.
fn decision(public: bool) -> cgrp::Candidate {
    let (s, command, visible) = if public {
        (
            support::ingroup(42),
            cgrp::Command::Fork {
                id: [3; 32],
                parents: vec![[2; 32]],
                members: members(0, 3),
                kind: cgrp::ForkKind::Open,
            },
            true,
        )
    } else {
        (
            cgrp::Snapshot::empty("test-community".into()),
            cgrp::Command::Create {
                id: [3; 32],
                members: members(0, 3),
            },
            false,
        )
    };
    let r = request(&s, command, "m0", members(0, 3), visible, 0);
    cgrp::prepare(
        &s,
        &r,
        &SignedPolicy::new(&policy()),
        &TestAuthority,
        0,
        128,
    )
    .unwrap()
}
fn beginning() -> Attend {
    let mut a = Attend::default();
    let c = decision(true);
    a.begin(
        &c.prepare_effects()[0],
        "m0",
        [1; 16],
        &SignedPolicy::new(&policy()),
        0,
    )
    .unwrap();
    a
}
// Explicit internal MLS/order preconditions, inaccessible in production. These
// exercise Attend coordination only; they are not MLS, proof or network tests.
struct Preconditions {
    ordering: [u8; 16],
    counter: u64,
    epoch: u64,
    empty: bool,
    refuse: bool,
}
impl Default for Preconditions {
    fn default() -> Self {
        Self {
            ordering: [2; 16],
            counter: 9,
            epoch: 2,
            empty: false,
            refuse: false,
        }
    }
}
impl Threads for Preconditions {
    fn prepare<'a>(&'a mut self, _: &'a [u8; 32], _: u64) -> PortFuture<'a, PreparedJoin> {
        Box::pin(async move {
            if self.refuse {
                return Err(Error::Unavailable);
            }
            Ok(PreparedJoin {
                ordering: self.ordering,
                expected_counter: 8,
                epoch: 2,
                encrypted_commit: if self.empty { vec![] } else { vec![1, 2, 3] },
                companion: vec![],
            })
        })
    }
    fn verify_order<'a>(
        &'a mut self,
        _: &'a OrderRequest,
        _: &'a OrderReply,
    ) -> PortFuture<'a, OrderedJoin> {
        Box::pin(async move {
            if self.refuse {
                return Err(Error::Unavailable);
            }
            Ok(OrderedJoin {
                ordering: self.ordering,
                counter: self.counter,
                epoch: self.epoch,
                encrypted_handover: vec![vec![7, 8, 9]],
            })
        })
    }
}
#[derive(Default)]
struct LinkBoundary {
    sent: Vec<Vec<u8>>,
    refuse: bool,
    ordering: Option<[u8; 16]>,
    counter: Option<u64>,
}
impl Link for LinkBoundary {
    fn order<'a>(&'a mut self, r: &'a OrderRequest) -> PortFuture<'a, OrderReply> {
        Box::pin(async move {
            self.sent.push(serde_json::to_vec(r).unwrap());
            if self.refuse {
                return Err(Error::Unavailable);
            }
            Ok(OrderReply {
                ordering: self.ordering.unwrap_or(r.ordering),
                counter: self.counter.unwrap_or(9),
                proof: vec![],
            })
        })
    }
}
struct Database {
    store: cwst::Store<cwst::backend::Memory>,
    backend: cwst::backend::Memory,
}
impl Database {
    async fn new() -> Self {
        let backend = cwst::backend::Memory::new();
        let store = cwst::Store::create(backend.clone(), &cipher(), "test-community")
            .await
            .unwrap();
        Self { store, backend }
    }
    fn checkpoint(&self) -> Checkpoint {
        postcard::from_bytes(self.store.read("attend", b"state").unwrap().unwrap()).unwrap()
    }
}
fn cipher() -> ckmg::passkey::StoreCipher {
    ckmg::passkey::store_cipher(
        &ckmg::passkey::PrfOutput::from_bytes(&[41; 32]).unwrap(),
        "test-community",
    )
    .unwrap()
}
impl Persistence for Database {
    fn commit<'a>(
        &'a mut self,
        expected: u64,
        c: &'a Checkpoint,
        outputs: &'a [Vec<u8>],
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let bytes = postcard::to_allocvec(c).unwrap();
            let operation = *ckmg::hash(&bytes).value();
            let out: Vec<_> = outputs
                .iter()
                .map(|b| cwst::Output {
                    id: *ckmg::hash(b).value(),
                    bytes: b.clone(),
                })
                .collect();
            self.store
                .commit(
                    expected,
                    operation,
                    &[cwst::Change::Put {
                        owner: "attend",
                        key: b"state",
                        value: &bytes,
                    }],
                    &out,
                )
                .await
                .map_err(|_| Error::Storage)?;
            Ok(())
        })
    }
}
vector!(
    signed_group_preparation_fences_consent_and_current_policy,
    {
        let p = SignedPolicy::new(&policy());
        let mut a = Attend::new();
        let c = decision(true);
        let plans = c.prepare_effects();
        assert_eq!(
            a.begin(&plans[0], "unknown", [1; 16], &p, 0),
            Err(Error::Authority)
        );
        let expired = support::room(42);
        let tick = request(
            &expired,
            cgrp::Command::Tick,
            "m0",
            std::collections::BTreeSet::new(),
            false,
            21 * cgrp::DAY,
        );
        let ended =
            cgrp::prepare(&expired, &tick, &p, &TestAuthority, 21 * cgrp::DAY, 128).unwrap();
        assert_eq!(
            a.begin(
                &ended.prepare_effects()[0],
                "m0",
                [1; 16],
                &p,
                21 * cgrp::DAY
            ),
            Err(Error::Authority)
        );
        let private = decision(false);
        assert_eq!(
            a.begin(&private.prepare_effects()[0], "m0", [1; 16], &p, 0),
            Err(Error::Authority)
        );
        let mut corrupt = SignedPolicy::new(&policy());
        corrupt.corrupt();
        assert_eq!(
            a.begin(&plans[0], "m0", [1; 16], &corrupt, 0),
            Err(Error::Authority)
        );
        assert_eq!(
            a.begin(&plans[0], "m0", [1; 16], &p, u64::MAX),
            Err(Error::Authority)
        );
        let mut invalid = policy();
        invalid.cap = 0;
        assert_eq!(
            a.begin(&plans[0], "m0", [1; 16], &SignedPolicy::new(&invalid), 0),
            Err(Error::Authority)
        );
        let mut changed = policy();
        changed.epoch = 2;
        assert_eq!(
            a.begin(&plans[0], "m0", [1; 16], &SignedPolicy::new(&changed), 0),
            Err(Error::Authority)
        );
        a.begin(&plans[0], "m0", [1; 16], &p, 0).unwrap();
        assert_eq!(
            a.begin(&plans[0], "m0", [1; 16], &p, 0),
            Err(Error::Invalid)
        );
        let mut db = Database::new().await;
        assert_eq!(
            a.prepare_join(&mut UnavailableThreads, &mut db, &p, 0)
                .await,
            Err(Error::Unavailable)
        );
        assert_eq!(a.phase(), &Phase::ConsentPending);
    }
);
vector!(real_encrypted_checkpoint_precedes_retry_and_handover, {
    let p = SignedPolicy::new(&policy());
    let mut a = beginning();
    let mut db = Database::new().await;
    let mut threads = Preconditions::default();
    a.prepare_join(&mut threads, &mut db, &p, 0).await.unwrap();
    assert_eq!(db.checkpoint().phase, Phase::Joining);
    assert_eq!(db.checkpoint().phase(), &Phase::Joining);
    assert_eq!(
        db.checkpoint().local_revision(),
        db.checkpoint().local_revision
    );
    let raw = db.backend.read().await.unwrap().unwrap();
    assert!(!raw.windows(14).any(|x| x == b"test-community"));
    let mut link = LinkBoundary::default();
    let reply = a.order(&mut link, &p, 0).await.unwrap();
    a.admitting_peer_offline().unwrap();
    a.order(&mut link, &p, 0).await.unwrap();
    assert_eq!(link.sent[0], link.sent[1]);
    assert_eq!(
        a.accept(&reply, &mut threads, &mut db, &p, 0)
            .await
            .unwrap(),
        vec![vec![7, 8, 9]]
    );
    assert_eq!(a.phase(), &Phase::Joined);
    let reopened = cwst::Store::open(db.backend.clone(), &cipher(), "test-community")
        .await
        .unwrap();
    assert_eq!(reopened.load_pending().unwrap()[0].bytes, vec![7, 8, 9]);
    a.withdraw(&mut db).await.unwrap();
    assert_eq!(a.phase(), &Phase::Withdrawn);
    let c = decision(true);
    a.begin(&c.prepare_effects()[0], "m0", [1; 16], &p, 0)
        .unwrap();
});
vector!(
    a_stored_checkpoint_restores_for_withdrawal_and_a_lost_attempt_never_resumes,
    {
        let p = SignedPolicy::new(&policy());
        let mut a = beginning();
        let mut db = Database::new().await;
        let mut threads = Preconditions::default();
        a.prepare_join(&mut threads, &mut db, &p, 0).await.unwrap();
        // A Joining checkpoint is a lost in-memory operation: reconcile, never resume.
        let lost = Attend::restore(db.checkpoint());
        assert_eq!(lost.phase(), &Phase::Reconciling);
        let mut link = LinkBoundary::default();
        let reply = a.order(&mut link, &p, 0).await.unwrap();
        a.accept(&reply, &mut threads, &mut db, &p, 0)
            .await
            .unwrap();
        let mut restored = Attend::restore(db.checkpoint());
        assert_eq!(restored.phase(), &Phase::Joined);
        assert_eq!(restored.checkpoint().room(), [1; 16]);
        restored.withdraw(&mut db).await.unwrap();
        assert_eq!(restored.phase(), &Phase::Withdrawn);
    }
);
vector!(
    a_companion_travels_with_the_commit_and_malformed_frames_are_refused,
    {
        assert_eq!(pack_ordered(&[0, 1, 9], &[]), vec![0, 1, 9]);
        assert_eq!(unpack_ordered(&[0, 1, 9]), Some((&[0, 1, 9][..], &[][..])));
        let frame = pack_ordered(&[0, 1, 9], &[5, 6]);
        assert_eq!(unpack_ordered(&frame), Some((&[0, 1, 9][..], &[5, 6][..])));
        // Magic with no companion, a zero or overlong commit length, a short header.
        let mut bad = pack_ordered(&[0, 1, 9], &[5]);
        bad.pop();
        assert_eq!(unpack_ordered(&bad), None);
        assert_eq!(unpack_ordered(&frame[..frame.len() - 2]), None);
        assert_eq!(unpack_ordered(&frame[..20]), None);
        assert!(
            PreparedJoin::new([1; 16], 0, vec![1], 1)
                .unwrap()
                .with_companion(vec![0; MAX_COMPANION + 1])
                .is_err()
        );
    }
);
vector!(phase_and_proof_refusals_never_create_join_authority, {
    let p = SignedPolicy::new(&policy());
    let mut db = Database::new().await;
    let mut threads = Preconditions::default();
    let mut link = LinkBoundary::default();
    let reply = OrderReply {
        ordering: [2; 16],
        counter: 9,
        proof: vec![],
    };
    let mut a = Attend::new();
    assert_eq!(
        a.prepare_join(&mut threads, &mut db, &p, 0).await,
        Err(Error::Invalid)
    );
    assert!(matches!(
        a.order(&mut link, &p, 0).await,
        Err(Error::Invalid)
    ));
    assert_eq!(
        a.accept(&reply, &mut threads, &mut db, &p, 0).await,
        Err(Error::Invalid)
    );
    assert_eq!(a.admitting_peer_offline(), Err(Error::Invalid));
    a = beginning();
    threads.empty = true;
    assert_eq!(
        a.prepare_join(&mut threads, &mut db, &p, 0).await,
        Err(Error::Authority)
    );
    threads.empty = false;
    a.prepare_join(&mut threads, &mut db, &p, 0).await.unwrap();
    a.admitting_peer_offline().unwrap();
    link.refuse = true;
    assert!(matches!(
        a.order(&mut link, &p, 0).await,
        Err(Error::Unavailable)
    ));
    link.refuse = false;
    a.order(&mut link, &p, 0).await.unwrap();
    a.order(&mut link, &p, 0).await.unwrap();
    assert_eq!(
        a.accept(&reply, &mut UnavailableThreads, &mut db, &p, 0)
            .await,
        Err(Error::Unavailable)
    );
    for (counter, epoch) in [(10, 2), (9, 3)] {
        threads.counter = counter;
        threads.epoch = epoch;
        a.checkpoint.phase = Phase::AwaitingOrderedCommit;
        assert_eq!(
            a.accept(&reply, &mut threads, &mut db, &p, 0).await,
            Err(Error::Reconcile)
        );
        assert_eq!(a.withdraw(&mut db).await, Err(Error::Reconcile));
    }
});
vector!(policy_changes_and_real_writer_conflict_block_publication, {
    let p = SignedPolicy::new(&policy());
    let mut db = Database::new().await;
    let mut a = beginning();
    let mut t = Preconditions::default();
    let mut corrupt = SignedPolicy::new(&policy());
    corrupt.corrupt();
    assert_eq!(
        a.prepare_join(&mut t, &mut db, &corrupt, 0).await,
        Err(Error::Authority)
    );
    assert_eq!(
        a.prepare_join(&mut t, &mut db, &p, u64::MAX).await,
        Err(Error::Authority)
    );
    let mut changed = policy();
    changed.epoch = 2;
    assert_eq!(
        a.prepare_join(&mut t, &mut db, &SignedPolicy::new(&changed), 0)
            .await,
        Err(Error::Authority)
    );
    a.prepare_join(&mut t, &mut db, &p, 0).await.unwrap();
    let reply = a.order(&mut LinkBoundary::default(), &p, 0).await.unwrap();
    let mut rival = cwst::Store::open(db.backend.clone(), &cipher(), "test-community")
        .await
        .unwrap();
    rival
        .commit(
            1,
            [91; 32],
            &[cwst::Change::Put {
                owner: "another",
                key: b"state",
                value: b"race",
            }],
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        a.accept(&reply, &mut t, &mut db, &p, 0).await,
        Err(Error::Storage)
    );
    assert_eq!(a.phase(), &Phase::Reconciling);
    let mut db = Database::new().await;
    let mut a = beginning();
    a.checkpoint.local_revision = u64::MAX;
    assert_eq!(a.withdraw(&mut db).await, Err(Error::Invalid));
});
vector!(history_respects_circle_window_order_and_message_cap, {
    let mut b = policy().bands[3].clone();
    b.history_seconds = 100;
    b.history_messages = 2;
    assert_eq!(
        history_window(Level::Circle, &b, 200, &[199]).unwrap(),
        Vec::<usize>::new()
    );
    assert_eq!(
        history_window(Level::Public, &b, 200, &[101, 100]),
        Err(Error::Invalid)
    );
    assert_eq!(
        history_window(Level::Public, &b, 200, &[99, 100, 101, 199, 200, 201]).unwrap(),
        vec![3, 4]
    );
    assert_eq!(
        history_window(Level::Public, &b, 200, &[199]).unwrap(),
        vec![0]
    );
    b.history_messages = 0;
    assert!(
        history_window(Level::Public, &b, 200, &[199])
            .unwrap()
            .is_empty()
    );
});

vector!(same_epoch_policy_renewal_does_not_extend_pending_consent, {
    let original = SignedPolicy::new(&policy());
    let mut renewed = policy();
    renewed.valid_until *= 2;
    let renewed = SignedPolicy::new(&renewed);
    let c = decision(true);
    let mut a = Attend::new();
    assert_eq!(
        a.begin(&c.prepare_effects()[0], "m0", [1; 16], &renewed, cgrp::DAY),
        Err(Error::Authority)
    );
    a.begin(&c.prepare_effects()[0], "m0", [1; 16], &original, 0)
        .unwrap();
    let mut db = Database::new().await;
    let mut threads = Preconditions::default();
    assert_eq!(
        a.prepare_join(&mut threads, &mut db, &renewed, cgrp::DAY)
            .await,
        Err(Error::Authority)
    );
    a.prepare_join(&mut threads, &mut db, &original, 0)
        .await
        .unwrap();
    assert_eq!(db.checkpoint().preparation_valid_until, cgrp::DAY);
    let mut link = LinkBoundary::default();
    assert!(matches!(
        a.order(&mut link, &renewed, cgrp::DAY).await,
        Err(Error::Authority)
    ));
    let reply = a.order(&mut link, &original, cgrp::DAY - 1).await.unwrap();
    assert_eq!(
        a.accept(&reply, &mut threads, &mut db, &renewed, cgrp::DAY)
            .await,
        Err(Error::Authority)
    );
    assert!(db.store.load_pending().unwrap().is_empty());
    // Successfully committed outputs retain their replay semantics; a policy
    // deadline only restricts pending authorization, not delivery of that output.
    a.accept(&reply, &mut threads, &mut db, &original, cgrp::DAY - 1)
        .await
        .unwrap();
    assert_eq!(db.store.load_pending().unwrap().len(), 1);
});
struct PendingWrite;
impl Persistence for PendingWrite {
    fn commit<'a>(&'a mut self, _: u64, _: &'a Checkpoint, _: &'a [Vec<u8>]) -> PortFuture<'a, ()> {
        Box::pin(std::future::pending())
    }
}
vector!(cancelled_checkpoint_stays_in_reconciliation, {
    let mut a = beginning();
    let mut store = PendingWrite;
    {
        let mut pending = Box::pin(a.withdraw(&mut store));
        let waker = futures::task::noop_waker();
        let mut context = std::task::Context::from_waker(&waker);
        assert!(pending.as_mut().poll(&mut context).is_pending());
    }
    assert_eq!(a.phase(), &Phase::Reconciling);
});

vector!(volatile_restart_and_swapped_order_never_publish_a_join, {
    let policy = SignedPolicy::new(&policy());
    for (ordering, counter) in [([3; 16], 9), ([2; 16], 8), ([2; 16], 10)] {
        let mut attend = beginning();
        let mut database = Database::new().await;
        let mut threads = Preconditions::default();
        attend
            .prepare_join(&mut threads, &mut database, &policy, 0)
            .await
            .unwrap();
        assert_eq!(database.checkpoint().ordering, [2; 16]);
        let mut link = LinkBoundary {
            ordering: Some(ordering),
            counter: Some(counter),
            ..Default::default()
        };
        assert!(matches!(
            attend.order(&mut link, &policy, 0).await,
            Err(Error::Reconcile)
        ));
        assert_eq!(attend.phase(), &Phase::Reconciling);
        assert_eq!(attend.admitting_peer_offline(), Err(Error::Invalid));
        assert!(database.store.load_pending().unwrap().is_empty());
        assert_eq!(database.checkpoint().phase, Phase::Joining);
    }
    let mut attend = beginning();
    let mut database = Database::new().await;
    let mut threads = Preconditions {
        ordering: [0; 16],
        ..Default::default()
    };
    assert_eq!(
        attend
            .prepare_join(&mut threads, &mut database, &policy, 0)
            .await,
        Err(Error::Authority)
    );
    threads.ordering = [2; 16];
    attend
        .prepare_join(&mut threads, &mut database, &policy, 0)
        .await
        .unwrap();
    let reply = attend
        .order(&mut LinkBoundary::default(), &policy, 0)
        .await
        .unwrap();
    // Equal numeric counters from another generation cannot be substituted
    // between transport receipt and publication, even before MLS verification.
    let swapped = OrderReply {
        ordering: [3; 16],
        counter: reply.counter,
        proof: reply.proof.clone(),
    };
    assert_eq!(
        attend
            .accept(&swapped, &mut UnavailableThreads, &mut database, &policy, 0)
            .await,
        Err(Error::Reconcile)
    );
    assert!(database.store.load_pending().unwrap().is_empty());
    // Internal transition preconditions only: a successful future owner
    // verifier must return evidence for this exact generation as well.
    attend.checkpoint.phase = Phase::AwaitingOrderedCommit;
    threads.ordering = [3; 16];
    assert_eq!(
        attend
            .accept(&reply, &mut threads, &mut database, &policy, 0)
            .await,
        Err(Error::Reconcile)
    );
    assert_eq!(attend.phase(), &Phase::Reconciling);
    assert!(database.store.load_pending().unwrap().is_empty());
});

#[test]
fn join_evidence_constructors_refuse_empty_or_unbound_input() {
    assert!(PreparedJoin::new([0; 16], 0, vec![1], 1).is_err());
    assert!(PreparedJoin::new([1; 16], 0, vec![], 1).is_err());
    assert!(PreparedJoin::new([1; 16], 0, vec![1], 1).is_ok());
    assert!(OrderedJoin::new([0; 16], 1, 1, vec![]).is_err());
    assert!(OrderedJoin::new([1; 16], 0, 1, vec![]).is_err());
    assert!(OrderedJoin::new([1; 16], 1, 1, vec![vec![]]).is_err());
    assert!(OrderedJoin::new([1; 16], 1, 1, vec![vec![1]; MAX_HANDOVER_ITEMS + 1]).is_err());
    assert!(OrderedJoin::new([1; 16], 1, 1, vec![vec![1]]).is_ok());
}
