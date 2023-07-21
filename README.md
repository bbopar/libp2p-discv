## Playground Project

## Description

This project is a small playground for experimenting with Rust and libp2p. The goal is to test MDNS discovery for a blockchain protocol.

## Features

- Network communication: The project includes different parts of the network that are able to communicate with each other.
- Peer information: The PeerInfo struct holds information about connected peers, including their addresses, client version, connected point, and latest ping.
- Peer behavior: The PeerInfoBehaviour struct defines the behavior of peers, including ping, identify, and a map of peer IDs to peer information.
- Kademlia DHT: The project uses Kademlia DHT tables for distributed hash table functionality.
- MDNS discovery: MDNS is used for discovering other nodes on the local network.

### Code snippets: 

Here’s an example of the PeerInfo struct:

```Rust
pub struct PeerInfo {
    pub peer_addresses: HashSet<Multiaddr>,
    pub client_version: Option<String>,
    pub connected_point: ConnectedPoint,
    pub latest_ping: Option<Duration>,
}
```


And here’s an example of the PeerInfoBehaviour struct:

```Rust
pub struct PeerInfoBehaviour {
    ping: Ping,
    identify: Identify,
    peers: HashMap<PeerId, PeerInfo>,
}
```
