pub mod behaviour;
pub mod config;
pub mod discovery;
pub mod peer_info;
use crate::{discovery::DiscoveryEvent, peer_info::PeerInfoEvent};
use behaviour::{Behaviour, GRNBehaviourEvent};
use config::Config;
use futures::StreamExt;
use libp2p::{
    core::{
        identity::{secp256k1, Keypair},
        multiaddr::{Multiaddr, Protocol},
        muxing::StreamMuxerBox,
        transport::Boxed,
        upgrade,
    },
    gossipsub::{GossipsubEvent, IdentTopic},
    mplex, noise,
    swarm::{ConnectionLimits, SwarmBuilder, SwarmEvent},
    yamux, PeerId, Swarm, Transport, TransportError,
};
use log::*;
use std::{
    fs::File,
    io::{prelude::*, Error},
};
use tokio::io::{self, AsyncBufReadExt};

pub const NETWORK_KEY_FILENAME: &str = "key";

/// Loads a private key from disk. If this fails, a new key is
/// generated and is then saved to disk.
pub fn create_or_load_private_key(config: &Config) -> Keypair {
    // check for key from disk
    let network_key_file_path = config.network_dir.join(NETWORK_KEY_FILENAME);
    if let Ok(mut network_key_file) = File::open(&network_key_file_path) {
        let mut key_bytes: [u8; 32] = [0; 32];
        match network_key_file.read(&mut key_bytes[..]) {
            Err(_) => debug!("Could not read network key file"),
            Ok(_) => {
                // only accept secp256k1 keys for now
                if let Ok(secret_key) = secp256k1::SecretKey::from_bytes(&mut key_bytes) {
                    let key_pair: secp256k1::Keypair = secret_key.into();
                    debug!("Loaded network key from disk.");
                    return Keypair::Secp256k1(key_pair);
                } else {
                    debug!("Network key file is not a valid secp256k1 key");
                }
            }
        }
    }

    // if a key could not be loaded from disk, generate a new one and save it
    let local_private_key = Keypair::generate_secp256k1();

    if let Keypair::Secp256k1(key) = &local_private_key {
        let _ = std::fs::create_dir_all(&config.network_dir);
        match File::create(&network_key_file_path)
            .and_then(|mut file| file.write_all(&key.secret().to_bytes()))
        {
            Ok(_) => {
                debug!("New network key generated and written to disk");
            }
            Err(e) => {
                warn!(
                    "Could not write node key to file: {:?}. error: {}",
                    network_key_file_path, e
                );
            }
        }
    }
    local_private_key
}

pub fn create_peer_id(keypair: &Keypair) -> PeerId {
    PeerId::from(keypair.public())
}

pub fn build_transport(
    local_keypair: &Keypair,
) -> std::io::Result<Boxed<(PeerId, StreamMuxerBox)>> {
    let transport = {
        let dns_tcp = libp2p::dns::TokioDnsConfig::system(libp2p::tcp::TokioTcpTransport::new(
            libp2p::tcp::GenTcpConfig::new().nodelay(true),
        ))?;
        let ws_dns_tcp = libp2p::websocket::WsConfig::new(libp2p::dns::TokioDnsConfig::system(
            libp2p::tcp::TokioTcpTransport::new(libp2p::tcp::GenTcpConfig::new().nodelay(true)),
        )?);
        dns_tcp.or_transport(ws_dns_tcp)
    };

    Ok(transport
        .upgrade(upgrade::Version::V1)
        .authenticate(generate_noise_config(local_keypair))
        .multiplex(upgrade::SelectUpgrade::new(
            yamux::YamuxConfig::default(),
            mplex::MplexConfig::default(),
        ))
        .timeout(std::time::Duration::from_secs(10))
        .boxed())
}

pub fn build_swarm(
    transport: Boxed<(PeerId, StreamMuxerBox)>,
    behaviour: Behaviour,
    local_peer_id: PeerId,
) -> Swarm<Behaviour> {
    SwarmBuilder::new(transport, behaviour, local_peer_id)
        .notify_handler_buffer_size(std::num::NonZeroUsize::new(7).expect("Not zero"))
        .connection_event_buffer_size(64)
        // Uncomment when network limits have been configured
        // .connection_limits(set_connection_limits())
        // Uncomment when custom executor is created
        // .executor(Box::new(Executor(executor)))
        .executor(Box::new(|fut| {
            tokio::spawn(fut);
        }))
        .build()
}

pub fn swarm_listener(
    swarm: &mut Swarm<Behaviour>,
    local_peer_id: PeerId,
    config: &Config,
) -> std::result::Result<(), TransportError<std::io::Error>> {
    let listen_multiaddr = {
        let mut m = Multiaddr::from(config.listen_address);
        m.push(Protocol::Tcp(config.libp2p_port));
        m
    };

    match Swarm::listen_on(swarm, listen_multiaddr.clone()) {
        Ok(_) => {
            let mut log_address = listen_multiaddr;
            log_address.push(Protocol::P2p(local_peer_id.into()));
            info!("Listening established address: {:?}", log_address);
            Ok(())
        }
        Err(err) => {
            error!(
                "Unable to listen on libp2p address
                error: {:?},
                listen_multiaddr {:?}",
                err, listen_multiaddr,
            );
            Err(err)
        }
    }
}

pub async fn event_loop(swarm: &mut Swarm<Behaviour>) -> Result<(), Error> {
    let mut stdin = io::BufReader::new(io::stdin()).lines();
    let topic = IdentTopic::new("chat");

    loop {
        tokio::select! {
            line = stdin.next_line() => {
                let line = line?.expect("stdin closed");
                // TODO narkomani obrisite ovo
                info!("DOBIO SAM LINIJU: {}", line);
                swarm.behaviour_mut().gossipsub.publish(topic.clone(), line.as_bytes()).unwrap();
            }
            event = swarm.select_next_some() => {
                match event {
                SwarmEvent::Behaviour(GRNBehaviourEvent::Discovery(event)) => match event {
                    DiscoveryEvent::Connected(peer_id, addresses) => {
                        info!("DiscoveryEvent::Connected peer_id: {:}", peer_id);
                        info!("DiscoveryEvent::Connected peer_id: {:?}", addresses);
                        swarm
                            .behaviour_mut()
                            .add_addresses_to_peer_info(&peer_id, addresses);
                    }
                    DiscoveryEvent::Disconnected(peer_id) => {
                        info!("DiscoveryEvent::Disconnected peer_id: {:}", peer_id);
                    }
                    _ => {}
                },
                SwarmEvent::Behaviour(GRNBehaviourEvent::Gossipsub(event)) => {
                    info!("NetworkEvent::Gossipsub {:?}", event);
                    match event {
                        GossipsubEvent::Message { propagation_source, message_id, message } => {
                            info!("Gossipsub event: `Message`, propagation_source: {:}", propagation_source);
                            info!("Gossipsub event: `Message`, message_id: {:}", message_id);
                            info!("Gossipsub event: `Message`, message: {:?}", message);
                        },
                        GossipsubEvent::Subscribed { peer_id, topic } => {
                            info!("Gossipsub event: `Subscribed`, peer_id: {:}", peer_id);
                            info!("Gossipsub event: `Subscribed`, topic: {:}", topic);
                        },
                        GossipsubEvent::Unsubscribed { peer_id, topic } => {
                            info!("Gossipsub event: `Unsubscribed`, peer_id: {:}", peer_id);
                            info!("Gossipsub event: `Unsubscribed`, topic: {:}", topic);
                        },
                        GossipsubEvent::GossipsubNotSupported { peer_id } => {
                            info!("Gossipsub event: `GossipsubNotSupported`, peer_id: {:}", peer_id);
                        },
                    }
                },
                SwarmEvent::Behaviour(GRNBehaviourEvent::PeerInfo(event)) => match event {
                    PeerInfoEvent::PeerIdentified { peer_id, addresses } => {
                        swarm
                            .behaviour_mut()
                            .add_addresses_to_discovery(&peer_id, addresses);
                    }
                    PeerInfoEvent::PeerInfoUpdated { peer_id } => {
                        info!("PeerInfoUpdated event with peer id: {}", peer_id);
                    }
                },
                _ => {}
            }
        }
        }
    }
}

pub fn connect_to_boot_nodes(swarm: &mut Swarm<Behaviour>, config: &Config) {
    // helper closure for dialing peers
    let mut dial = |mut multiaddr: Multiaddr| {
        // strip the p2p protocol if it exists
        strip_peer_id(&mut multiaddr);
        match Swarm::dial(swarm, multiaddr.clone()) {
            Ok(()) => debug!("Dialing libp2p peer address {}", multiaddr),
            Err(err) => debug!(
                "Could not connect to peer address {} with error {}",
                multiaddr, err
            ),
        };
    };

    // attempt to connect to user-input libp2p nodes
    for multiaddr in &config.libp2p_nodes {
        dial(multiaddr.clone());
    }
}

/// For a multiaddr that ends with a peer id, this strips this suffix. Rust-libp2p
/// only supports dialing to an address without providing the peer id.
fn strip_peer_id(addr: &mut Multiaddr) {
    let last = addr.pop();
    match last {
        Some(Protocol::P2p(_)) => {}
        Some(other) => addr.push(other),
        _ => {}
    }
}

/// Generate authenticated XX Noise config from identity keys
fn generate_noise_config(
    identity_keypair: &Keypair,
) -> noise::NoiseAuthenticated<noise::XX, noise::X25519Spec, ()> {
    let static_dh_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(identity_keypair)
        .expect("signing can fail only once during starting a node");
    noise::NoiseConfig::xx(static_dh_keys).into_authenticated()
}

// No need for current usage. Will be used in future for better network optimization.
// Just an example -> Numbers should be researched and tested for better outcome.
#[allow(dead_code)]
fn set_connection_limits() -> ConnectionLimits {
    ConnectionLimits::default()
        .with_max_pending_incoming(Some(5))
        .with_max_pending_outgoing(Some(16))
        .with_max_established_incoming(Some(50))
        .with_max_established_outgoing(Some(50))
        .with_max_established(Some(55))
        .with_max_established_per_peer(Some(1))
}
