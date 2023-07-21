use log::*;
use network::{behaviour::Behaviour, config::Config};

#[tokio::main]
async fn main() {
    logger::setup();
    let mut config = Config::default();

    if let Some(address_arg) = std::env::args().nth(1) {
        config
            .libp2p_nodes
            .push(address_arg.parse().expect("to input a valid multiaddrs"));
    }

    let local_keypair = network::create_or_load_private_key(&config);
    let peer_id = network::create_peer_id(&local_keypair);
    info!("Current peer id: {:}", peer_id);

    let mut swarm = {
        let transport = network::build_transport(&local_keypair).expect("Transport to start");

        let behaviour = Behaviour::new(&local_keypair);

        network::build_swarm(transport, behaviour, peer_id)
    };

    network::swarm_listener(&mut swarm, peer_id, &config).unwrap();

    if !config.libp2p_nodes.is_empty() {
        network::connect_to_boot_nodes(&mut swarm, &config);
    }

    let _ = network::event_loop(&mut swarm).await;

    info!("GrnGRID logger");
}
