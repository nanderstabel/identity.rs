use communication_refactored::{InitKeypair, Keypair};
use libp2p::{tcp::TcpConfig, Multiaddr};

use crate::{actor_builder::ActorBuilder, asyncfn::AsyncFn, storage::requests::IdentityList, StorageHandler};

#[tokio::test]
async fn test_list_identities() -> anyhow::Result<()> {
  let id_keys = Keypair::generate_ed25519();
  let transport = TcpConfig::new().nodelay(true);

  let addr: Multiaddr = "/ip4/127.0.0.1/tcp/1337".parse().unwrap();

  let mut listening_actor = ActorBuilder::new()
    .keys(InitKeypair::IdKeys(id_keys))
    .listen_on(addr.clone())
    .build_with_transport(transport)
    .await?;

  let handler = StorageHandler::new().await?;

  listening_actor
    .add_handler(handler)
    .add_method("storage/list", AsyncFn::new(StorageHandler::list))
    .add_method(
      "storage/list2",
      AsyncFn::new(|_obj: StorageHandler, _req: IdentityList| async { vec![] }),
    )
    .add_method("storage/resolve", AsyncFn::new(StorageHandler::resolve));

  let peer_id = listening_actor.peer_id();

  let mut sending_actor = ActorBuilder::new().build().await?;
  sending_actor.add_peer(peer_id, addr).await;

  let result = sending_actor.send_request(peer_id, IdentityList).await?;

  assert!(result.is_empty());

  listening_actor.stop_handling_requests().await.unwrap();

  Ok(())
}
