use super::*;
use crate::common::{chain, fallback_committee, fallback_keys, MockMempool};
use crypto::SecretKey;
use std::fs;
use tokio::sync::mpsc::channel;
use threshold_crypto::SecretKeySet;

async fn fallback(
    name: PublicKey,
    secret: SecretKey,
    store_path: &str,
) -> (Sender<CoreMessage>, Receiver<NetMessage>, Receiver<Block>) {
    let (tx_core, rx_core) = channel(1);
    let (tx_network, rx_network) = channel(3);
    let (tx_commit, rx_commit) = channel(1);

    let parameters = Parameters {
        timeout_delay: 100,
        ..Parameters::default()
    };
    let signature_service = SignatureService::new(secret, None);
    let _ = fs::remove_dir_all(store_path);
    let store = Store::new(store_path).unwrap();
    let leader_elector = LeaderElector::new(fallback_committee());
    let mempool_driver = MempoolDriver::new(MockMempool, tx_core.clone(), store.clone());
    let synchronizer = Synchronizer::new(
        name,
        fallback_committee(),
        store.clone(),
        /* network_channel */ tx_network.clone(),
        /* core_channel */ tx_core.clone(),
        parameters.sync_retry_delay,
    )
    .await;
    let size = fallback_committee().size();
    let threshold = (size - 1) / 3 + 1;
    let mut rng = rand::thread_rng();
    let sk_set = SecretKeySet::random(threshold, &mut rng);
    let pk_set = sk_set.public_keys();
    
    let mut fallback = Fallback::new(
        name,
        fallback_committee(),
        parameters,
        signature_service,
        pk_set,
        store,
        leader_elector,
        mempool_driver,
        synchronizer,
        /* core_channel */ rx_core,
        /* network_channel */ tx_network,
        /* commit_channel */ tx_commit,
    );
    tokio::spawn(async move {
        fallback.run().await;
    });
    (tx_core, rx_network, rx_commit)
}

fn leader_keys(round: SeqNumber) -> (PublicKey, SecretKey) {
    let leader_elector = LeaderElector::new(fallback_committee());
    let leader = leader_elector.get_leader(round);
    fallback_keys()
        .into_iter()
        .find(|(public_key, _)| *public_key == leader)
        .unwrap()
}

#[tokio::test]
async fn handle_proposal() {
    // Make a block and the vote we expect to receive.
    let block = chain(vec![leader_keys(1)]).pop().unwrap();
    let (public_key, secret_key) = fallback_keys().pop().unwrap();
    let vote = Vote::new_from_key(block.digest(), block.view, block.round, block.height, block.fallback, block.author, public_key, &secret_key);

    // Run a core instance.
    let store_path = ".db_test_handle_proposal_fallback";
    let (tx_core, mut rx_network, _rx_commit) = fallback(public_key, secret_key, store_path).await;

    // Send a block to the core.
    let message = CoreMessage::Propose(block.clone());
    tx_core.send(message).await.unwrap();

    // Ensure we get a vote back.
    match rx_network.recv().await {
        Some(NetMessage(bytes, recipient)) => {
            match bincode::deserialize(&bytes).unwrap() {
                CoreMessage::Vote(v) => assert_eq!(v, vote),
                _ => assert!(false),
            }
            let (next_leader, _) = leader_keys(2);
            let address = fallback_committee().address(&next_leader).unwrap();
            assert_eq!(recipient, vec![address]);
        }
        _ => assert!(false),
    }
}

#[tokio::test]
async fn generate_proposal() {
    // Get the keys of the leaders of this round and the next.
    let (leader, leader_key) = leader_keys(1);
    let (next_leader, next_leader_key) = leader_keys(2);

    // Make a block, votes, and QC.
    let block = Block::new_from_key(QC::genesis(), leader, 1, 1, 0, 0, Vec::new(), &leader_key);
    let hash = block.digest();
    let votes: Vec<_> = fallback_keys()
        .iter()
        .map(|(public_key, secret_key)| {
            Vote::new_from_key(hash.clone(), block.view, block.round, block.height, block.fallback, block.author, *public_key, &secret_key)
        })
        .collect();
    let qc = QC {
        hash,
        view: block.view,
        round: block.round,
        height: block.height,
        fallback: block.fallback,
        proposer: block.author,
        votes: votes
            .iter()
            .cloned()
            .map(|x| (x.author, x.signature))
            .collect(),
    };

    // Run a core instance.
    let store_path = ".db_test_generate_proposal_fallback";
    let (tx_core, mut rx_network, _rx_commit) =
        fallback(next_leader, next_leader_key, store_path).await;

    // Send all votes to the core.
    for vote in votes.clone() {
        let message = CoreMessage::Vote(vote);
        tx_core.send(message).await.unwrap();
    }

    // Ensure the core sends a new block.
    match rx_network.recv().await {
        Some(NetMessage(bytes, mut recipients)) => {
            match bincode::deserialize(&bytes).unwrap() {
                CoreMessage::Propose(b) => {
                    assert_eq!(b.round, 2);
                    assert_eq!(b.qc, qc);
                }
                _ => assert!(false),
            }
            let mut addresses = fallback_committee().broadcast_addresses(&next_leader);
            addresses.sort();
            recipients.sort();
            assert_eq!(recipients, addresses);
        }
        _ => assert!(false),
    }
}

#[tokio::test]
async fn commit_block() {
    // Get 3 successive blocks.
    let leaders = vec![leader_keys(1), leader_keys(2), leader_keys(3)];
    let chain = chain(leaders);

    // Run a core instance.
    let store_path = ".db_test_commit_block_fallback";
    let (public_key, secret_key) = fallback_keys().pop().unwrap();
    let (tx_core, _rx_network, mut rx_commit) = fallback(public_key, secret_key, store_path).await;

    // Send a 3-chain to the core.
    for block in chain.clone() {
        let message = CoreMessage::Propose(block);
        tx_core.send(message).await.unwrap();
    }

    // Ensure the core commits the head.
    match rx_commit.recv().await {
        Some(b) => assert_eq!(b, Block::genesis()),
        _ => assert!(false),
    }
}

#[tokio::test]
async fn local_timeout_round() {
    // Make the timeout vote we expect.
    let (public_key, secret_key) = leader_keys(3);
    let timeout = Timeout::new_from_key(QC::genesis(), 0, public_key, &secret_key);

    // Run a core instance.
    let store_path = ".db_test_local_timeout_round_fallback";
    let (_tx_core, mut rx_network, _rx_commit) = fallback(public_key, secret_key, store_path).await;

    // Ensure the following operation happen in the right order.
    match rx_network.recv().await {
        Some(NetMessage(bytes, mut recipients)) => {
            match bincode::deserialize(&bytes).unwrap() {
                CoreMessage::Timeout(t) => assert_eq!(t, timeout),
                _ => assert!(false),
            }
            let mut addresses = fallback_committee().broadcast_addresses(&public_key);
            addresses.sort();
            recipients.sort();
            assert_eq!(recipients, addresses);
        }
        _ => assert!(false),
    }
}