//! Voxel social-art-space "game" you can have some fun in.
#![allow(incomplete_features)]
#![feature(extract_if)]
#![feature(str_from_raw_parts)]
#![feature(string_remove_matches)]
#![feature(generic_const_exprs)]
#![feature(int_roundings)]
#![feature(inherent_associated_types)]
#![feature(array_try_from_fn)]
#![feature(trivial_bounds)]
#![allow(clippy::large_enum_variant)]

#[macro_use]
pub mod common;
pub mod main_channels;
use clap::Parser;
pub use common::message;
use net::{generated::get_netmsg_table, NetMsg, PacketIntermediary};
pub use crate::main_channels::*;
use semver::Version;

#[macro_use]
pub mod net;

#[macro_use]
pub mod resource;

//pub mod client;
pub mod entity;
pub mod message_types;
pub mod script;
pub mod server;
pub mod world;

use std::{
	io::Write,
	net::{IpAddr, Ipv6Addr, SocketAddr},
	path::PathBuf,
	time::Duration,
};

use log::{error, info, warn, LevelFilter};
use simplelog::{
	ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger,
};

use common::{
	identity::{do_keys_need_generating, gen_and_save_keys, load_keyfile, NodeIdentity},
	message::*
};

use crate::{
	message::QuitReceiver,
	message_types::{
		voxel::{VoxelChangeAnnounce, VoxelChangeRequest},
		JoinAnnounce, JoinDefaultEntry,
	},
	net::{
		default_protocol_store_dir,
		preprotocol::{launch_preprotocol_listener, preprotocol_connect_to_server},
		reliable_udp::LaminarConfig,
		NetworkSystem, SelfNetworkRole,
	},
};

pub const ENGINE_VERSION: Version = Version::new(0,0,1);

pub async fn protocol_key_change_approver(
	mut receiver: BroadcastReceiver<NodeIdentity>,
	sender: BroadcastSender<(NodeIdentity, bool)>,
) {
	loop {
		match receiver.recv_wait().await {
			Ok(ident) => {
				warn!(
					"Protocol key has changed for peer {:?} - most likely this is the same user \n\
				connecting with a new device, but it's possible it's an attempt to impersonate them.",
					ident.to_base64()
				);
				//Approve implicitly.
				//When GUI is a thing, we want this to generate a popup for clients.
				sender.send((ident.clone(), true)).unwrap();
			}
			Err(e) => panic!("Protocol key change approver channel died: {:?}", e),
		}
	}
}

pub fn init_channels() -> MainChannelSet { 
	let conf = ChannelCapacityConf::new(); 
	MainChannelSet::new(&conf)
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    server: bool,
    #[arg(short, long)]
	join: bool,
    #[arg(short, long)]
	addr: Option<String>,
    #[arg(short, long)]
	verbose: bool,
}

#[allow(unused_must_use)]
fn main() {
	// Announce the engine launching, for our command-line friends.
	println!("Launching Gestalt Engine v{}", ENGINE_VERSION);

	let program_args = Args::parse();

	//Initialize our logger.
	let mut log_config_builder = ConfigBuilder::default();

	let level_filter = {
		if program_args.verbose {
			// Verbosely log Gestalt messages but try not to verbosely log renderer messages because they can get ridiculous.
			log_config_builder.add_filter_ignore_str("wgpu");
			log_config_builder.add_filter_ignore_str("rend3");
			log_config_builder.add_filter_ignore_str("naga");
			log_config_builder.set_location_level(LevelFilter::Error);
			LevelFilter::Trace
		} else {
			log_config_builder.add_filter_ignore_str("wgpu_core::device");
			LevelFilter::Info
		}
	};

	log_config_builder.set_target_level(level_filter);

	let log_config = log_config_builder.build();

	let log_dir = PathBuf::from("logs/");
	let log_file_path = log_dir.join("latest.log");

	if !log_dir.exists() {
		std::fs::create_dir(log_dir);
	}

	CombinedLogger::init(vec![
		TermLogger::new(level_filter, log_config.clone(), TerminalMode::Mixed, ColorChoice::Auto),
		WriteLogger::new(level_filter, log_config, std::fs::File::create(log_file_path).unwrap()),
	])
	.unwrap();

	if matches!(level_filter, LevelFilter::Trace) {
		warn!("Verbose logging CAN, OCCASIONALLY, LEAK PRIVATE INFORMATION. \n It is only recommended for debugging purposes. \n Please do not use it for general play.");
	}

	info!("Initializing main channel set...");
	let channels = init_channels();
	for (net_msg_id, _) in get_netmsg_table() { 
		channels.net_channels.net_msg_inbound.init_domain(*net_msg_id);
	}
	info!("Main channel set ready.");

	let key_dir = PathBuf::from("keys/");
	let keyfile_name = "identity_key";
	// Load our identity key pair. Right now this will be the same on both client and server - that will change later.
	// Using environment variables here might also be a good move.
	let keys = if do_keys_need_generating(key_dir.clone(), keyfile_name) {
		println!("No identity keys found, generating identity keys.");
		println!("Optionally enter a passphrase.");
		println!("Minimum length is 4 characters.");
		println!("WARNING: If you forget your passphrase, this will be impossible to recover!");
		println!("Leave this blank if you do not want to use a passphrase.");
		let mut input = String::new();
		loop { 
			print!("Enter your passphrase: ");
			std::io::stdout().flush().unwrap();

			std::io::stdin()
				.read_line(&mut input)
				.expect("Error reading from STDIN");
			print!("Confirm your passphrase: ");
			std::io::stdout().flush().unwrap();
			let mut confirm = String::new();
			std::io::stdin()
				.read_line(&mut confirm)
				.expect("Error reading from STDIN");
			if confirm == input { 
				break;
			} else {
				println!("Passphrases do not match! Please try again.");
				input = String::new();
			}
		}

		// If it's 1 char, that's a newline or a \0
		let passphrase = if input.chars().count() > 1 {
			Some(input.as_str())
		} else {
			None
		};

		gen_and_save_keys(passphrase, key_dir.clone(), keyfile_name).unwrap()
	} else {
		let key_file = load_keyfile(key_dir.clone(), keyfile_name).unwrap();
		let passphrase = if key_file.needs_passphrase() {
			println!("Your identity key is encrypted. Please enter your passphrase.");
			print!("Passphrase: ");
			std::io::stdout().flush().unwrap();

			let mut input = String::new();
			std::io::stdin()
				.read_line(&mut input)
				.expect("Error reading from STDIN");
			Some(input)
		} else {
			None
		};
		key_file
			.try_read(passphrase.as_ref().map(|v| v.as_str()))
			.unwrap()
	};

	info!("Identity keys loaded! Initializing engine...");

	info!("Setting up async runtime.");
	let mut runtime_builder = tokio::runtime::Builder::new_multi_thread();
	runtime_builder.enable_all();

	let async_runtime = match runtime_builder.build() {
		Ok(rt) => rt,
		Err(e) => {
			error!("Unable to start async runtime: {:?}", e);
			panic!("Unable to start async runtime: {:?}", e);
		}
	};

	info!("Setting up channels.");

	async_runtime.spawn(protocol_key_change_approver(
		channels.net_channels.key_mismatch_reporter.receiver_subscribe(),
		channels.net_channels.key_mismatch_approver.sender_subscribe(),
	));

	let mut laminar_config = LaminarConfig::default();
	laminar_config.heartbeat_interval = Some(Duration::from_secs(1));

	let protocol_store_dir = default_protocol_store_dir();

	if program_args.server {
		info!("Launching as server - parsing address.");
		let udp_address = if let Some(raw_addr) = program_args.addr {
			if raw_addr.contains(':') {
				raw_addr.parse().unwrap()
			} else {
				let ip_addr: IpAddr = raw_addr.parse().unwrap();
				SocketAddr::new(ip_addr, 3223)
			}
		} else {
			SocketAddr::from((Ipv6Addr::LOCALHOST, 3223))
		};


		let preprotocol_channels = channels.net_channels.build_subset(SubsetBuilder::new(())).unwrap();
		info!("Spawning preprotocol listener task.");
		async_runtime.spawn(launch_preprotocol_listener(
			keys,
			None,
			3223,
			protocol_store_dir,
			preprotocol_channels,
		));

		info!("Spawning network system task.");
		let keys_for_net = keys.clone();
		let net_channels = channels.net_channels.build_subset(SubsetBuilder::new(())).unwrap();
		let net_system_join_handle = async_runtime.spawn(async move {
			let mut sys = NetworkSystem::new(
				SelfNetworkRole::Server,
				udp_address,
				keys_for_net,
				laminar_config,
				Duration::from_millis(25),
				net_channels
			)
			.await
			.unwrap();
			sys.run().await
		});

		//let test_world_range: VoxelRange<i32> = VoxelRange{upper: vpos!(3,3,3), lower: vpos!(-2,-2,-2) };
		//let mut world_space = TileSpace::new();
		//for chunk_position in test_world_range {
		//    let chunk = gen_test_chunk(chunk_position);
		//    world_space.ingest_loaded_chunk(chunk_position, chunk).unwrap();
		//}

		// Set up our test world a bit
		//let mut world_space = TileSpace::new();
		//let test_world_range: VoxelRange<i32> = VoxelRange{upper: vpos!(3,3,3), lower: vpos!(-2,-2,-2) };

		//let world_id = get_lobby_world_id(&keys.public);
		//load_or_generate_dev_world(&mut world_space, &world_id, test_world_range, None).unwrap();

		info!("Launching server mainloop.");
		let mut total_changes: Vec<VoxelChangeAnnounce> = Vec::new();
		let net_channels = channels.net_channels.clone();
		async_runtime.block_on(async move {
			let mut quit_receiver = QuitReceiver::new();
			let mut voxel_from_client =
				net_channels.net_msg_inbound.receiver_typed::<VoxelChangeAnnounce>().unwrap();
			let mut joins_to_server =
				net_channels.net_msg_inbound.receiver_typed::<JoinDefaultEntry>().unwrap();
			let net_msg_broadcast = net_channels.net_msg_outbound.sender_subscribe_all();
			loop {
				tokio::select! {
					voxel_events_maybe = voxel_from_client.recv_wait() => {
						if let Ok(voxel_events) = voxel_events_maybe {
							for (ident, event) in voxel_events {
								//world_space.set(event.pos, event.new_tile).unwrap();
								info!("Received {:?} from {}", &event, ident.to_base64());
								let announce: VoxelChangeAnnounce = event.into();
								net_msg_broadcast.send_to_all_except(vec![announce.clone().construct_packet().unwrap()], &ident).unwrap();
								total_changes.push(announce);
							}
						}
					}
					join_event_maybe = joins_to_server.recv_wait() => {
						if let Ok(events) = join_event_maybe {
							for (ident, event) in events {
								info!("User {} has joined with display name {}", ident.to_base64(), &event.display_name);
								let announce = JoinAnnounce {
									display_name: event.display_name,
									identity: ident,
								};
								net_msg_broadcast.send_to_all_except(vec![announce.clone().construct_packet().unwrap()], &ident).unwrap();
								info!("Sending all previous changes to the newly-joined user.");

								let sender_to_new_join = net_channels.net_msg_outbound.sender_subscribe_domain(&ident).unwrap();
								sender_to_new_join.send(
									total_changes
										.iter()
										.map(|ev| ev.construct_packet().unwrap())
										.collect::<Vec<PacketIntermediary>>()
								).unwrap();
							}
						}
					}
					quit_ready_indicator = quit_receiver.wait_for_quit() => {
						quit_ready_indicator.notify_ready();
						break;
					}
				}
			}
		});
		message::quit_game(Duration::from_secs(10));
		async_runtime.block_on(net_system_join_handle);
	} else if let Some(raw_addr) = {
		if program_args.join {
			program_args.addr
		} else {
			None
		}
	} {
		info!("Launching as client");
		let address: SocketAddr = if raw_addr.contains(':') {
			raw_addr.parse().unwrap()
		} else {
			let ip_addr: IpAddr = raw_addr.parse().unwrap();
			SocketAddr::new(ip_addr, 3223)
		};

		let keys_for_net = keys.clone();
		let net_channels = channels.net_channels.build_subset(SubsetBuilder::new(())).unwrap();
		let net_system_join_handle = async_runtime.spawn(async move {
			let mut sys = NetworkSystem::new(
				SelfNetworkRole::Client,
				address,
				keys_for_net,
				laminar_config,
				Duration::from_millis(25),
				net_channels
			)
			.await
			.unwrap();
			sys.run().await
		});
		async_runtime
			.block_on(preprotocol_connect_to_server(
				keys,
				address,
				Duration::new(5, 0),
				protocol_store_dir,
				channels.net_channels.build_subset(SubsetBuilder::new(())).unwrap()
			))
			.unwrap();
		let mut connect_receiver = channels.net_channels.peer_connected.receiver_subscribe();
		let _completed = async_runtime.block_on( async { 
			connect_receiver.recv_wait().await
		}).unwrap();

		std::thread::sleep(Duration::from_millis(50));

		let mut peer_joins_notif = channels.net_channels.net_msg_inbound.receiver_typed::<JoinAnnounce>().unwrap();

		async_runtime.spawn(async move {
			loop {
				match peer_joins_notif.recv_wait().await {
					Ok(join_msgs) => {
						for (
							_server_ident,
							JoinAnnounce {
								identity,
								display_name,
							},
						) in join_msgs
						{
							info!(
								"Peer {} joined with display name {}",
								identity.to_base64(),
								&display_name
							);
						}
					}
					Err(_e) => {
						info!("New join handler closed.");
						break;
					}
				}
			}
		});

		async_runtime.spawn(async move {
			let mut quit_receiver = QuitReceiver::new();
			let quit_ready = quit_receiver.wait_for_quit().await;
			net_system_join_handle.await; //This is why quit_ready_sender exists. Make sure that's all done.
			quit_ready.notify_ready();
		});
		/*
		client::clientmain::run_client(
			keys,
			voxel_event_sender,
			client_voxel_receiver_from_server,
			Some(server_identity),
			async_runtime,
		);*/
	} else {
		info!("Launching as stand-alone.");
		let mut voxel_event_receiver = channels.net_channels.net_msg_inbound.receiver_typed::<VoxelChangeRequest>().unwrap();

		async_runtime.spawn(async move {
			loop {
				//redirect to /dev/null
				let _ = voxel_event_receiver.recv_wait().await;
			}
		}); /*
		let client_voxel_receiver_from_server =
			net_recv_channel::subscribe::<VoxelChangeAnnounce>().unwrap();
		 client::clientmain::run_client(
			 keys,
			 voxel_event_sender,
			 client_voxel_receiver_from_server,
			 None,
			 async_runtime,
		 );*/
	}
}
