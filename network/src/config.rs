use libp2p::gossipsub::{FastMessageId, GossipsubMessage, MessageId, RawGossipsubMessage};
use libp2p::identity::Keypair;
use libp2p::{
    gossipsub::{GossipsubConfig, GossipsubConfigBuilder},
    Multiaddr,
};
use serde_derive::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::{
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

/// Names for the default directories.
pub const DEFAULT_ROOT_DIR: &str = ".grngrid";
pub const DEFAULT_NETWORK_DIR: &str = "network";
pub const DEFAULT_VALIDATOR_DIR: &str = "validators";
pub const DEFAULT_SECRET_DIR: &str = "secrets";
pub const DEFAULT_WALLET_DIR: &str = "wallets";
pub const DEFAULT_HARDCODED_NETWORK: &str = "localnet";
pub const NETWORK_KEY_FILENAME: &str = "key";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
/// Network configuration for network.
pub struct Config {
    /// Data directory where node's keyfile is stored
    pub network_dir: PathBuf,

    /// IP address to listen on.
    pub listen_address: std::net::IpAddr,

    /// List of libp2p nodes to initially connect to.
    pub libp2p_nodes: Vec<Multiaddr>,

    /// The TCP port that libp2p listens on.
    pub libp2p_port: u16,

    /// UDP port that discovery listens on.
    pub discovery_port: u16,

    /// Gossipsub configuration parameters.
    #[serde(skip)]
    pub gs_config: GossipsubConfig,
}

impl Default for Config {
    /// Generate a default network configuration.
    fn default() -> Self {
        // WARNING: this directory default should be always overwritten with parameters
        // from cli for specific networks.
        let network_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(DEFAULT_ROOT_DIR)
            .join(DEFAULT_HARDCODED_NETWORK)
            .join(DEFAULT_NETWORK_DIR);

        // Note: Using the default config here. Use `gossipsub_config` function for getting
        // specific configuration for gossipsub.
        let gs_config = GossipsubConfigBuilder::default()
            .build()
            .expect("valid gossipsub configuration");

        // NOTE: Some of these get overridden by the corresponding CLI default values.
        Config {
            network_dir,
            listen_address: "0.0.0.0".parse().expect("valid ip address"),
            libp2p_port: 9000,
            discovery_port: 9000,
            libp2p_nodes: vec![],
            gs_config,
        }
    }
}

// TODO set this
pub const NEW_TX_GOSSIP_TOPIC: &str = "new_tx";
pub const NEW_BLOCK_GOSSIP_TOPIC: &str = "new_block";
pub const CON_VOTE_GOSSIP_TOPIC: &str = "consensus_vote";

const REQ_RES_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug)]
pub struct P2PConfig {
    pub local_keypair: Keypair,

    /// Name of the Network
    pub network_name: String,

    /// IP address for Swarm to listen on
    pub address: IpAddr,

    /// The TCP port that Swarm listens on
    pub tcp_port: u16,

    /// Max Size of a block in bytes
    pub max_block_size: usize,

    // `DiscoveryBehaviour` related fields
    pub bootstrap_nodes: Vec<Multiaddr>,
    pub enable_mdns: bool,
    pub max_peers_connected: usize,
    pub allow_private_addresses: bool,
    pub enable_random_walk: bool,
    pub connection_idle_timeout: Option<Duration>,

    // `PeerInfo` fields
    /// The interval at which identification requests are sent to
    /// the remote on established connections after the first request
    pub identify_interval: Option<Duration>,
    /// The duration between the last successful outbound or inbound ping
    /// and the next outbound ping
    pub info_interval: Option<Duration>,

    // `Gossipsub` config and topics
    pub gossipsub_config: GossipsubConfig,
    pub topics: Vec<String>,

    // RequestResponse related fields
    /// Sets the timeout for inbound and outbound requests.
    pub set_request_timeout: Duration,
    /// Sets the keep-alive timeout of idle connections.
    pub set_connection_keep_alive: Duration,

    /// Enables prometheus metrics
    pub metrics: bool,
}

impl P2PConfig {
    pub fn default_with_network(network_name: &str) -> Self {
        let local_keypair = Keypair::generate_secp256k1();

        P2PConfig {
            local_keypair,
            network_name: network_name.into(),
            address: IpAddr::V4(Ipv4Addr::from([0, 0, 0, 0])),
            tcp_port: 0,
            max_block_size: 100_000,
            bootstrap_nodes: vec![],
            enable_mdns: true,
            max_peers_connected: 50,
            allow_private_addresses: true,
            enable_random_walk: true,
            connection_idle_timeout: Some(Duration::from_secs(120)),
            topics: vec![
                NEW_TX_GOSSIP_TOPIC.into(),
                NEW_BLOCK_GOSSIP_TOPIC.into(),
                CON_VOTE_GOSSIP_TOPIC.into(),
            ],
            gossipsub_config: default_gossipsub_config(),
            set_request_timeout: REQ_RES_TIMEOUT,
            set_connection_keep_alive: REQ_RES_TIMEOUT,
            info_interval: Some(Duration::from_secs(3)),
            identify_interval: Some(Duration::from_secs(5)),
            metrics: false,
        }
    }
}

/// Creates `GossipsubConfigBuilder` with few of the Gossipsub values already defined
pub fn default_gossipsub_builder() -> GossipsubConfigBuilder {
    let gossip_message_id =
        move |message: &GossipsubMessage| MessageId::from(&Sha256::digest(&message.data)[..]);

    let fast_gossip_message_id = move |message: &RawGossipsubMessage| {
        FastMessageId::from(&Sha256::digest(&message.data)[..])
    };

    let mut builder = GossipsubConfigBuilder::default();

    builder
        .protocol_id_prefix("/meshsub/1.0.0")
        .message_id_fn(gossip_message_id)
        .fast_message_id_fn(fast_gossip_message_id)
        .validate_messages();

    builder
}

pub(crate) fn default_gossipsub_config() -> GossipsubConfig {
    default_gossipsub_builder()
        .mesh_n(6)
        .mesh_n_low(4)
        .mesh_n_high(12)
        .build()
        .expect("valid gossipsub configuration")
}
