use std::collections::HashMap;

use crate::config::P2PConfig;
use crate::discovery::{DiscoveryBehaviour, DiscoveryConfig, DiscoveryEvent};
use crate::peer_info::{PeerInfo, PeerInfoBehaviour, PeerInfoEvent};
use libp2p::gossipsub;
use libp2p::gossipsub::{
    error::{PublishError, SubscriptionError},
    Gossipsub, GossipsubEvent, MessageAcceptance, MessageAuthenticity, MessageId, Sha256Topic,
    ValidationMode,
};
use libp2p::identity::Keypair;
use libp2p::Multiaddr;
use libp2p::{NetworkBehaviour, PeerId};

pub type GossipTopic = Sha256Topic;

pub enum GRNBehaviourEvent {
    PeerInfo(PeerInfoEvent),
    Gossipsub(GossipsubEvent),
    Discovery(DiscoveryEvent),
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "GRNBehaviourEvent")]
pub struct Behaviour {
    pub peer_info: PeerInfoBehaviour,
    pub gossipsub: Gossipsub,
    pub discovery: DiscoveryBehaviour,
}

impl Behaviour {
    pub fn new(local_key: &Keypair) -> Behaviour {
        let local_peer_id = PeerId::from(local_key.public());

        // TODO add this to gossipsub file -> [
        // Create a Gossipsub topic
        let topic = gossipsub::IdentTopic::new("chat");

        // Set a custom gossipsub
        let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
            .heartbeat_interval(std::time::Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
            .support_floodsub()
            .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
            .build()
            .expect("Valid config");

        // Build a gossipsub network behaviour
        let mut gossipsub: gossipsub::Gossipsub = gossipsub::Gossipsub::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )
        .expect("Correct configuration");

        // Subscribes to our topic
        gossipsub.subscribe(&topic).unwrap();

        // ] end of gossipsub inst.

        let p2p_config = P2PConfig::default_with_network("GRN_network");

        let discovery_config = {
            let mut discovery_config =
                DiscoveryConfig::new(local_peer_id, p2p_config.network_name.clone());

            discovery_config
                .enable_mdns(p2p_config.enable_mdns)
                .discovery_limit(p2p_config.max_peers_connected)
                .allow_private_addresses(p2p_config.allow_private_addresses)
                .with_bootstrap_nodes(p2p_config.bootstrap_nodes.clone())
                .enable_random_walk(p2p_config.enable_random_walk);

            if let Some(duration) = p2p_config.connection_idle_timeout {
                discovery_config.set_connection_idle_timeout(duration);
            }

            discovery_config
        };

        let peer_info = PeerInfoBehaviour::new(local_key.public(), &p2p_config);

        Behaviour {
            peer_info,
            gossipsub,
            discovery: discovery_config.finish(),
        }
    }

    pub fn add_addresses_to_peer_info(&mut self, peer_id: &PeerId, addresses: Vec<Multiaddr>) {
        self.peer_info.insert_peer_addresses(peer_id, addresses);
    }

    pub fn add_addresses_to_discovery(&mut self, peer_id: &PeerId, addresses: Vec<Multiaddr>) {
        for address in addresses {
            self.discovery.add_address(peer_id, address.clone());
        }
    }

    pub fn get_peers(&self) -> &HashMap<PeerId, PeerInfo> {
        self.peer_info.peers()
    }

    pub fn publish_message(
        &mut self,
        topic: GossipTopic,
        encoded_data: Vec<u8>,
    ) -> Result<MessageId, PublishError> {
        self.gossipsub.publish(topic, encoded_data)
    }

    pub fn subscribe_to_topic(&mut self, topic: &GossipTopic) -> Result<bool, SubscriptionError> {
        self.gossipsub.subscribe(topic)
    }

    // pub fn send_request_msg(
    //     &mut self,
    //     message_request: RequestMessage,
    //     peer_id: PeerId,
    // ) -> RequestId {
    //     self.request_response
    //         .send_request(&peer_id, message_request)
    // }

    // pub fn send_response_msg(
    //     &mut self,
    //     channel: ResponseChannel<IntermediateResponse>,
    //     message: IntermediateResponse,
    // ) -> Result<(), IntermediateResponse> {
    //     self.request_response.send_response(channel, message)
    // }

    pub fn report_message_validation_result(
        &mut self,
        msg_id: &MessageId,
        propagation_source: &PeerId,
        acceptance: MessageAcceptance,
    ) -> Result<bool, PublishError> {
        self.gossipsub
            .report_message_validation_result(msg_id, propagation_source, acceptance)
    }

    // // Currently only used in testing, but should be useful for the NetworkOrchestrator API
    // #[allow(dead_code)]
    // pub fn get_peer_info(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
    //     self.peer_info.get_peer_info(peer_id)
    // }
}

impl From<DiscoveryEvent> for GRNBehaviourEvent {
    fn from(event: DiscoveryEvent) -> Self {
        GRNBehaviourEvent::Discovery(event)
    }
}

impl From<GossipsubEvent> for GRNBehaviourEvent {
    fn from(event: GossipsubEvent) -> Self {
        GRNBehaviourEvent::Gossipsub(event)
    }
}

impl From<PeerInfoEvent> for GRNBehaviourEvent {
    fn from(event: PeerInfoEvent) -> Self {
        GRNBehaviourEvent::PeerInfo(event)
    }
}
