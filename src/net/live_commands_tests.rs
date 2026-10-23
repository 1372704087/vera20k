use super::*;
use crate::net::reliable::LanConnectionConfig;
use crate::net::udp::UdpBindConfig;
use crate::rules::{ini_parser::IniFile, ruleset::RuleSet};
use crate::sim::command::{CommandEnvelope, CommandRecord};
use crate::sim::command_queue::{FrameChecksumState, FrameInfo};
use crate::sim::house_state::HouseState;
use crate::sim::scenario_session::ScenarioGameMode;
use crate::sim::world::{
    FrameEffects, NetworkCommandBoundary, NetworkCommandError, SimFrameOutput, Simulation, TickLane,
};
use std::cell::{Cell, RefCell};
use std::net::{Ipv4Addr, UdpSocket};
use std::time::{Duration, Instant};

fn transport() -> LanUdpTransport {
    LanUdpTransport::bind(
        UdpBindConfig {
            primary: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            alternate: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
        },
        LanConnections::new(
            LanConnectionConfig::default(),
            RetryTiming {
                retry_delta: 0,
                max_retries: u32::MAX,
                timeout: u32::MAX,
            },
        ),
    )
    .unwrap()
}

#[test]
fn wol_catchup_pacing_uses_admitted_peer_progress_and_existing_rtt() {
    let (mut live, _) = pair(73);
    live.mode = LiveCommandMode::Wol;
    let mut sim = Simulation::new();
    let packet = crate::net::event_packet::EventPacketCodec::default()
        .encode_packet(
            &[],
            crate::net::event_packet::EventPacketHeader::Vera {
                frame_info: FrameInfo {
                    house_id: 1,
                    event_frame: 0,
                    checksum: 0,
                    timing_word: 0,
                    delay: 0,
                },
                command_frontier: 0,
            },
            0,
            crate::net::event_packet::NATIVE_EVENT_PACKET_CAPACITY,
        )
        .unwrap()
        .bytes;
    assert!(
        live.exchange
            .receive(1, &packet, 73, &mut sim.network_command_queues(), 0)
            .unwrap()
    );
    live.exchange.record_peer_lag(1, 40, 60, 40);
    assert_eq!(live.peers()[0].lag_frames(), 20);
    assert_eq!(live.pacing_delay_millis(40), 20);
    assert_eq!(live.pacing_delay_millis(30), 0);
    assert!(
        !live
            .exchange
            .receive(1, &packet, 73, &mut sim.network_command_queues(), 0)
            .unwrap()
    );
    assert_eq!(live.pacing_delay_millis(40), 20);
    live.remove_peer(1);
    assert_eq!(live.pacing_delay_millis(40), 0);
}

fn pair(seed: u32) -> (LiveCommandSession, LiveCommandSession) {
    let mut left = transport();
    let mut right = transport();
    left.connections_mut()
        .add_peer(1, right.primary_addr().unwrap())
        .unwrap();
    right
        .connections_mut()
        .add_peer(0, left.primary_addr().unwrap())
        .unwrap();
    (
        LiveCommandSession::new(
            left,
            0,
            seed,
            NonZeroU32::new(5).unwrap(),
            LiveCommandMode::Lan,
        )
        .unwrap(),
        LiveCommandSession::new(
            right,
            1,
            seed,
            NonZeroU32::new(5).unwrap(),
            LiveCommandMode::Lan,
        )
        .unwrap(),
    )
}

fn poll_pair(
    left: &mut LiveCommandSession,
    right: &mut LiveCommandSession,
    left_world: &mut Simulation,
    right_world: &mut Simulation,
    frame: u32,
) -> (NetworkQueuePass, NetworkQueuePass) {
    let start = Instant::now();
    let context = NetworkQueueContext {
        frame,
        checksum: 0x12345678,
    };
    let mut passes = [NetworkQueuePass::Waiting; 2];
    while passes.contains(&NetworkQueuePass::Waiting) {
        let now = 1000 + (start.elapsed().as_millis() / 16) as u32;
        passes[0] = left
            .poll(
                &mut left_world.network_command_queues(),
                context,
                10,
                now,
                (now).wrapping_mul(16),
                0,
                0,
            )
            .unwrap();
        passes[1] = right
            .poll(
                &mut right_world.network_command_queues(),
                context,
                10,
                now,
                (now).wrapping_mul(16),
                0,
                0,
            )
            .unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "frame{frame} did not finish"
        );
        std::thread::yield_now();
    }
    (passes[0], passes[1])
}

#[test]
fn initial_barrier_off_cadence_and_positive_frame_use_one_native_queue() {
    let (mut left, mut right) = pair(73);
    let mut a = Simulation::new();
    let mut b = Simulation::new();
    assert_eq!(
        poll_pair(&mut left, &mut right, &mut a, &mut b, 0),
        (
            NetworkQueuePass::SkipDispatch,
            NetworkQueuePass::SkipDispatch
        )
    );
    let command = CommandRecord::encode(6, 0, 1, &[42, 0, 0, 0, 0x34]).unwrap();
    assert!(a.issue_local_command(CommandEnvelope::synchronized(command, 1)));
    for frame in 1..5 {
        assert_eq!(
            poll_pair(&mut left, &mut right, &mut a, &mut b, frame),
            (
                NetworkQueuePass::SkipDispatch,
                NetworkQueuePass::SkipDispatch
            )
        );
        assert_eq!(a.network_command_queues().local_len(), 1);
    }
    assert_eq!(
        poll_pair(&mut left, &mut right, &mut a, &mut b, 5),
        (NetworkQueuePass::Dispatch, NetworkQueuePass::Dispatch)
    );
    assert_eq!(left.sent_commands(), 1);
    assert_eq!(right.sent_commands(), 0);
    assert_eq!(a.network_command_queues().local_len(), 0);
    assert_eq!(b.network_command_queues().local_len(), 0);
    // The same frame result is idempotent and cannot create another send.
    assert_eq!(
        left.poll(
            &mut a.network_command_queues(),
            NetworkQueueContext {
                frame: 5,
                checksum: 0x12345678
            },
            10,
            1001,
            (1001_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Dispatch
    );
    assert_eq!(left.sent_commands(), 1);
    // Frame5 still has credit from the frame0 packet, so it may dispatch
    // before the other socket's frame5 packet arrives. At frame10 the strict
    // minimum-frame gate6495F9 requires newer progress; its command count
    // must also be satisfied before the delayed event can execute at15.
    for frame in 6..=10 {
        poll_pair(&mut left, &mut right, &mut a, &mut b, frame);
    }
    for world in [&a, &b] {
        let records = world.pending_command_snapshot();
        assert_eq!(
            records
                .iter()
                .filter(|event| matches!(&event.payload,
            crate::sim::command::Command::SynchronizedRecord { record }
            if record.opcode() == 6 && record.frame_stamp() == 15 && record.house_id() == 0))
                .count(),
            1
        );
    }
}

#[test]
fn initial_response_histories_clear_only_on_successful_barrier_once() {
    use crate::net::global::GlobalDestination;
    use crate::net::reliable::connection_prefix;

    fn seed_history(
        live: &mut LiveCommandSession,
        endpoint: SocketAddrV4,
        sequence: u32,
        now: u32,
    ) {
        assert!(live.transport.connections_mut().send_to(
            1,
            b"private history",
            Delivery::Reliable
        ));
        live.transport
            .connections_mut()
            .service(now, &mut |_, _| {});
        live.transport
            .connections_mut()
            .receive(
                endpoint,
                &connection_prefix(0xaa04, 2, sequence, 0),
                &mut |_, _| {},
            )
            .unwrap();
        live.transport
            .connections_mut()
            .service(now + 11, &mut |_, _| {});
        assert!(live.global_mut().send(
            b"global history",
            GlobalDestination::direct(endpoint),
            Delivery::Reliable,
            0
        ));
        live.global_mut().service(now, &mut |_, _| {});
        live.global_mut()
            .receive(
                endpoint,
                0,
                &connection_prefix(0x1236, 2, sequence, 2),
                now + 22,
                &mut |_, _| {},
            )
            .unwrap();
        live.global_mut().service(now + 22, &mut |_, _| {});
    }

    let mut wire = transport();
    let peer_socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let std::net::SocketAddr::V4(peer) = peer_socket.local_addr().unwrap() else {
        unreachable!()
    };
    wire.connections_mut().add_peer(1, peer).unwrap();
    let mut live = LiveCommandSession::new(
        wire,
        0,
        73,
        NonZeroU32::new(5).unwrap(),
        LiveCommandMode::Lan,
    )
    .unwrap();
    seed_history(&mut live, peer, 0, 100);
    let mut sim = Simulation::new();
    let context = NetworkQueueContext {
        frame: 0,
        checksum: 0x12345678,
    };
    for now in [1000, 1001] {
        assert_eq!(
            live.poll(
                &mut sim.network_command_queues(),
                context,
                10,
                now,
                now * 16,
                0,
                0
            )
            .unwrap(),
            NetworkQueuePass::Waiting
        );
        assert_eq!(live.connections().mean_response_time(1), Some(11));
        assert_eq!(live.global().mean_response_time(), 22);
    }
    let probe = live
        .exchange
        .frame_probe(1, 0, 10, 73, live.policy)
        .unwrap();
    live.exchange
        .receive(1, &probe, 73, &mut sim.network_command_queues(), 1)
        .unwrap();
    assert_eq!(
        live.poll(
            &mut sim.network_command_queues(),
            context,
            10,
            1002,
            16032,
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::SkipDispatch
    );
    assert_eq!(live.connections().mean_response_time(1), Some(0));
    assert_eq!(live.global().mean_response_time(), 0);
    assert_eq!(live.peers()[0].frame(), 0);

    seed_history(&mut live, peer, 1, 1100);
    for next in [
        context,
        NetworkQueueContext {
            frame: 1,
            ..context
        },
    ] {
        assert_eq!(
            live.poll(
                &mut sim.network_command_queues(),
                next,
                10,
                1200,
                19200,
                0,
                0
            )
            .unwrap(),
            NetworkQueuePass::SkipDispatch
        );
        assert_eq!(live.connections().mean_response_time(1), Some(11));
        assert_eq!(live.global().mean_response_time(), 22);
    }
}

#[test]
fn ordinary_wait_uses_private_response_history_with_larger_primary_global_mean() {
    use crate::net::global::GlobalDestination;
    use crate::net::reliable::connection_prefix;

    let mut wire = transport();
    let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let std::net::SocketAddr::V4(peer) = receiver.local_addr().unwrap() else {
        unreachable!()
    };
    wire.connections_mut().add_peer(1, peer).unwrap();
    let mut live = LiveCommandSession::new(
        wire,
        0,
        73,
        NonZeroU32::new(5).unwrap(),
        LiveCommandMode::Lan,
    )
    .unwrap();
    let mut sim = Simulation::new();
    let context = NetworkQueueContext {
        frame: 0,
        checksum: 0x12345678,
    };
    assert_eq!(
        live.poll(
            &mut sim.network_command_queues(),
            context,
            5,
            1000,
            16000,
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Waiting
    );
    let probe = live.exchange.frame_probe(1, 0, 5, 73, live.policy).unwrap();
    live.exchange
        .receive(1, &probe, 73, &mut sim.network_command_queues(), 1)
        .unwrap();
    assert_eq!(
        live.poll(
            &mut sim.network_command_queues(),
            context,
            5,
            1001,
            16016,
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::SkipDispatch
    );

    assert!(
        live.transport
            .connections_mut()
            .send_to(1, b"private sample", Delivery::Reliable)
    );
    live.transport
        .connections_mut()
        .service(1100, &mut |_, _| {});
    live.transport
        .connections_mut()
        .receive(peer, &connection_prefix(0xaa04, 2, 0, 0), &mut |_, _| {})
        .unwrap();
    live.transport
        .connections_mut()
        .service(1110, &mut |_, _| {});
    assert!(live.global_mut().send(
        b"global sample",
        GlobalDestination::direct(peer),
        Delivery::Reliable,
        0
    ));
    live.global_mut().service(1100, &mut |_, _| {});
    live.global_mut()
        .receive(
            peer,
            0,
            &connection_prefix(0x1236, 2, 0, 2),
            1300,
            &mut |_, _| {},
        )
        .unwrap();
    live.global_mut().service(1300, &mut |_, _| {});
    assert_eq!(live.connections().mean_response_time(1), Some(10));
    assert_eq!(live.global().mean_response_time(), 200);
    for frame in 1..5 {
        assert_eq!(
            live.poll(
                &mut sim.network_command_queues(),
                NetworkQueueContext { frame, ..context },
                5,
                1300 + frame,
                (1300 + frame) * 16,
                0,
                0
            )
            .unwrap(),
            NetworkQueuePass::SkipDispatch
        );
    }
    assert_eq!(
        live.poll(
            &mut sim.network_command_queues(),
            NetworkQueueContext {
                frame: 5,
                ..context
            },
            5,
            1305,
            20880,
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Waiting
    );
    assert_eq!(
        live.wait_info(),
        Some(LiveWaitInfo {
            mode: LiveCommandMode::Lan,
            kind: LiveWaitKind::Ordinary,
            entered_at: 1305,
            response_time_at_entry: 10,
        })
    );
}

#[test]
fn online_initial_probe_waits_for_the_online_table_boundary() {
    let mut wire = transport();
    let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let std::net::SocketAddr::V4(peer) = receiver.local_addr().unwrap() else {
        unreachable!()
    };
    wire.connections_mut().add_peer(1, peer).unwrap();
    let mut live = LiveCommandSession::new(
        wire,
        0,
        73,
        NonZeroU32::new(5).unwrap(),
        LiveCommandMode::Wol,
    )
    .unwrap();
    let mut sim = Simulation::new();
    let context = NetworkQueueContext {
        frame: 0,
        checksum: 1,
    };
    live.poll(
        &mut sim.network_command_queues(),
        context,
        10,
        1000,
        16000,
        0,
        0,
    )
    .unwrap();
    live.take_service_report();
    live.poll(
        &mut sim.network_command_queues(),
        context,
        10,
        1119,
        17904,
        0,
        0,
    )
    .unwrap();
    assert_eq!(live.take_service_report().unwrap().sent_datagrams, 0);
    live.poll(
        &mut sim.network_command_queues(),
        context,
        10,
        1120,
        17920,
        0,
        0,
    )
    .unwrap();
    assert_eq!(live.take_service_report().unwrap().sent_datagrams, 1);
    assert_eq!(live.wait_info().unwrap().mode, LiveCommandMode::Wol);
}

#[test]
fn initial_probe_repeats_only_at_the_native_wall_clock_boundary() {
    let mut wire = transport();
    let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let address = match receiver.local_addr().unwrap() {
        std::net::SocketAddr::V4(a) => a,
        _ => unreachable!(),
    };
    wire.connections_mut().add_peer(1, address).unwrap();
    let mut live = LiveCommandSession::new(
        wire,
        0,
        91,
        NonZeroU32::new(5).unwrap(),
        LiveCommandMode::Lan,
    )
    .unwrap();
    let mut sim = Simulation::new();
    let context = NetworkQueueContext {
        frame: 0,
        checksum: 10,
    };
    assert_eq!(
        live.poll(
            &mut sim.network_command_queues(),
            context,
            10,
            1000,
            (1000_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Waiting
    );
    assert_eq!(
        live.wait_info(),
        Some(LiveWaitInfo {
            mode: LiveCommandMode::Lan,
            kind: LiveWaitKind::Initial,
            entered_at: 1000,
            response_time_at_entry: 0,
        })
    );
    let mut bytes = [0; 640];
    let (length, _) = receiver.recv_from(&mut bytes).unwrap();
    assert_eq!(
        super::super::event_packet::decode_frame_sync(&bytes[18..length])
            .unwrap()
            .checksum,
        91
    );
    assert_eq!(
        live.poll(
            &mut sim.network_command_queues(),
            context,
            10,
            1059,
            (1059_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Waiting
    );
    assert_eq!(live.take_service_report().unwrap().sent_datagrams, 0);
    assert_eq!(
        live.poll(
            &mut sim.network_command_queues(),
            context,
            10,
            1060,
            (1060_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Waiting
    );
    assert_eq!(live.take_service_report().unwrap().sent_datagrams, 1);
    let (length, _) = receiver.recv_from(&mut bytes).unwrap();
    assert!(super::super::event_packet::decode_frame_sync(&bytes[18..length]).is_some());
    assert!(live.restart_wait_probe(1100));
    assert_eq!(live.wait_info().unwrap().entered_at, 1000);
    live.poll(
        &mut sim.network_command_queues(),
        context,
        10,
        1159,
        (1159_u32).wrapping_mul(16),
        0,
        0,
    )
    .unwrap();
    assert_eq!(live.take_service_report().unwrap().sent_datagrams, 0);
    live.poll(
        &mut sim.network_command_queues(),
        context,
        10,
        1160,
        (1160_u32).wrapping_mul(16),
        0,
        0,
    )
    .unwrap();
    assert_eq!(live.take_service_report().unwrap().sent_datagrams, 1);
}

#[test]
fn ordinary_wait_uses_180_buckets_and_keeps_the_frame_send_complete() {
    let (mut left, mut right) = pair(73);
    let mut a = Simulation::new();
    let mut b = Simulation::new();
    poll_pair(&mut left, &mut right, &mut a, &mut b, 0);
    for frame in 1..10 {
        let pass = left
            .poll(
                &mut a.network_command_queues(),
                NetworkQueueContext {
                    frame,
                    checksum: 0x12345678,
                },
                10,
                2000,
                (2000_u32).wrapping_mul(16),
                0,
                0,
            )
            .unwrap();
        assert_eq!(
            pass,
            if frame == 5 {
                NetworkQueuePass::Dispatch
            } else {
                NetworkQueuePass::SkipDispatch
            }
        );
    }
    let context = NetworkQueueContext {
        frame: 10,
        checksum: 0x12345678,
    };
    assert_eq!(
        left.poll(
            &mut a.network_command_queues(),
            context,
            10,
            2000,
            (2000_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Waiting
    );
    assert_eq!(left.frame_wait().unwrap().missing_frame, Some(1));
    assert_eq!(
        left.wait_info(),
        Some(LiveWaitInfo {
            mode: LiveCommandMode::Lan,
            kind: LiveWaitKind::Ordinary,
            entered_at: 2000,
            response_time_at_entry: 0,
        })
    );
    assert_eq!(left.lowest_frame_peer().unwrap().house_id(), 1);
    assert_eq!(
        left.poll(
            &mut a.network_command_queues(),
            context,
            10,
            2179,
            (2179_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Waiting
    );
    assert_eq!(left.take_service_report().unwrap().sent_datagrams, 0);
    assert_eq!(
        left.poll(
            &mut a.network_command_queues(),
            context,
            10,
            2180,
            (2180_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Waiting
    );
    assert_eq!(left.take_service_report().unwrap().sent_datagrams, 1);
    assert_eq!(left.sent_commands(), 0);
    assert!(left.restart_wait_probe(2300));
    assert_eq!(left.wait_info().unwrap().entered_at, 2000);
    left.poll(
        &mut a.network_command_queues(),
        context,
        10,
        2479,
        (2479_u32).wrapping_mul(16),
        0,
        0,
    )
    .unwrap();
    assert_eq!(left.take_service_report().unwrap().sent_datagrams, 0);
    left.poll(
        &mut a.network_command_queues(),
        context,
        10,
        2480,
        (2480_u32).wrapping_mul(16),
        0,
        0,
    )
    .unwrap();
    assert_eq!(left.take_service_report().unwrap().sent_datagrams, 1);
}

#[test]
fn lowest_frame_projection_uses_existing_signed_progress_and_first_tie() {
    let mut wire = transport();
    for house in [3, 1, 2] {
        wire.connections_mut()
            .add_peer(
                house,
                SocketAddrV4::new(Ipv4Addr::LOCALHOST, 20000 + house as u16),
            )
            .unwrap();
    }
    let mut live = LiveCommandSession::new(
        wire,
        0,
        73,
        NonZeroU32::new(5).unwrap(),
        LiveCommandMode::Lan,
    )
    .unwrap();
    let mut sim = Simulation::new();
    assert_eq!(live.lowest_frame_peer().unwrap().house_id(), 3);
    assert_eq!(live.lowest_frame_peer().unwrap().frame(), -1);
    for (house, expected) in [(3, 1), (1, 2), (2, 3)] {
        let packet = live
            .exchange
            .frame_probe(house, 20, 0, 73, live.policy)
            .unwrap();
        live.exchange
            .receive(house, &packet, 73, &mut sim.network_command_queues(), 0)
            .unwrap();
        assert_eq!(live.lowest_frame_peer().unwrap().house_id(), expected);
    }
    assert_eq!(live.lowest_frame_peer().unwrap().frame(), 20);
}

#[test]
fn invalid_roster_context_and_initial_seed_are_explicit_failures() {
    // Native zero-remote loops finish immediately; LAN can start with one
    // human, and removing the last remote retains the same behavior.
    assert!(
        LiveCommandSession::new(
            transport(),
            0,
            1,
            NonZeroU32::new(5).unwrap(),
            LiveCommandMode::Lan
        )
        .is_ok()
    );
    let mut wire = transport();
    wire.connections_mut()
        .add_peer(200, SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1234))
        .unwrap();
    assert!(matches!(
        LiveCommandSession::new(
            wire,
            0,
            1,
            NonZeroU32::new(5).unwrap(),
            LiveCommandMode::Lan
        ),
        Err(LiveCommandError::InvalidRemoteHouse(200))
    ));
    let mut wire = transport();
    wire.connections_mut()
        .add_peer(1, SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
        .unwrap();
    assert!(matches!(
        LiveCommandSession::new(
            wire,
            0,
            1,
            NonZeroU32::new(5).unwrap(),
            LiveCommandMode::Lan
        ),
        Err(LiveCommandError::UnboundPeer { .. })
    ));
    let (mut left, mut right) = pair(73);
    right.initial_seed = 74;
    let mut a = Simulation::new();
    let mut b = Simulation::new();
    let context = NetworkQueueContext {
        frame: 0,
        checksum: 10,
    };
    assert!(matches!(
        left.poll(
            &mut a.network_command_queues(),
            NetworkQueueContext {
                frame: 1,
                checksum: 10
            },
            10,
            1000,
            (1000_u32).wrapping_mul(16),
            0,
            0
        ),
        Err(LiveCommandError::FrameSequence {
            expected: 0,
            actual: 1
        })
    ));
    let start = Instant::now();
    loop {
        let l = left.poll(
            &mut a.network_command_queues(),
            context,
            10,
            1000,
            (1000_u32).wrapping_mul(16),
            0,
            0,
        );
        let r = right.poll(
            &mut b.network_command_queues(),
            context,
            10,
            1000,
            (1000_u32).wrapping_mul(16),
            0,
            0,
        );
        if matches!(
            l,
            Err(LiveCommandError::Exchange(
                CommandExchangeError::InitialChecksum { .. }
            ))
        ) || matches!(
            r,
            Err(LiveCommandError::Exchange(
                CommandExchangeError::InitialChecksum { .. }
            ))
        ) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
}

#[test]
fn failed_retry_service_keeps_receiving_and_retains_its_diagnostic() {
    let (mut left, mut right) = pair(73);
    let mut a = Simulation::new();
    let mut b = Simulation::new();
    poll_pair(&mut left, &mut right, &mut a, &mut b, 0);
    for frame in 1..10 {
        left.poll(
            &mut a.network_command_queues(),
            NetworkQueueContext {
                frame,
                checksum: 0x12345678,
            },
            10,
            2000,
            (2000_u32).wrapping_mul(16),
            0,
            0,
        )
        .unwrap();
    }
    // A valid reliable message without an ACK fails this service attempt.
    // Native648B94/649518 still admit the peer's progress and test readiness.
    left.set_retry_timing(RetryTiming {
        retry_delta: 0,
        max_retries: 0,
        timeout: u32::MAX,
    });
    let pending = left
        .exchange
        .frame_probe(0, 10, 10, 73, left.policy)
        .unwrap();
    assert!(
        left.transport
            .connections_mut()
            .broadcast(&pending, Delivery::Reliable)
    );
    let progress = right
        .exchange
        .frame_probe(1, 20, 10, 73, right.policy)
        .unwrap();
    assert!(
        right
            .transport
            .connections_mut()
            .broadcast(&progress, Delivery::Unreliable)
    );
    right.transport.service(2001);
    let context = NetworkQueueContext {
        frame: 10,
        checksum: 0x12345678,
    };
    let start = Instant::now();
    let mut now = 2002;
    loop {
        let pass = left
            .poll(
                &mut a.network_command_queues(),
                context,
                10,
                now,
                (now).wrapping_mul(16),
                0,
                0,
            )
            .unwrap();
        assert_eq!(left.take_service_report().unwrap().failed_peer, Some(1));
        if pass == NetworkQueuePass::Dispatch {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(2));
        now = now.wrapping_add(1);
        std::thread::yield_now();
    }
    assert_eq!(left.peers()[0].frame(), 20);
    assert!(left.connections().queued_send_count(1).unwrap() > 0);
    assert_eq!(
        left.poll(
            &mut a.network_command_queues(),
            context,
            10,
            2003,
            (2003_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Dispatch
    );
}

#[test]
fn removing_the_last_peer_resumes_wait_and_keeps_local_transfer_running() {
    let (mut left, mut right) = pair(73);
    let mut a = Simulation::new();
    let mut b = Simulation::new();
    poll_pair(&mut left, &mut right, &mut a, &mut b, 0);
    for frame in 1..10 {
        left.poll(
            &mut a.network_command_queues(),
            NetworkQueueContext {
                frame,
                checksum: 0x12345678,
            },
            10,
            2000,
            (2000_u32).wrapping_mul(16),
            0,
            0,
        )
        .unwrap();
    }
    let command = CommandRecord::encode(6, 0, 10, &[42, 0, 0, 0, 0x34]).unwrap();
    assert!(a.issue_local_command(CommandEnvelope::synchronized(command, 10)));
    let context = NetworkQueueContext {
        frame: 10,
        checksum: 0x12345678,
    };
    assert_eq!(
        left.poll(
            &mut a.network_command_queues(),
            context,
            10,
            2001,
            (2001_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Waiting
    );
    assert!(left.remove_peer(1));
    assert!(left.peers().is_empty());
    assert_eq!(left.connections().peers().len(), 0);
    assert_eq!(
        left.poll(
            &mut a.network_command_queues(),
            context,
            10,
            2002,
            (2002_u32).wrapping_mul(16),
            0,
            0
        )
        .unwrap(),
        NetworkQueuePass::Dispatch
    );
    let command = CommandRecord::encode(6, 0, 11, &[43, 0, 0, 0, 0x34]).unwrap();
    assert!(a.issue_local_command(CommandEnvelope::synchronized(command, 11)));
    for frame in 11..=15 {
        assert_eq!(
            left.poll(
                &mut a.network_command_queues(),
                NetworkQueueContext {
                    frame,
                    checksum: 0x12345678,
                },
                10,
                2003,
                (2003_u32).wrapping_mul(16),
                0,
                0
            )
            .unwrap(),
            if frame == 15 {
                NetworkQueuePass::Dispatch
            } else {
                NetworkQueuePass::SkipDispatch
            }
        );
    }
    assert_eq!(left.sent_commands(), 2);
    assert_eq!(a.network_command_queues().local_len(), 0);
    for frame in [20, 25] {
        assert!(a.pending_command_snapshot().iter().any(|command| {
            matches!(&command.payload,
                crate::sim::command::Command::SynchronizedRecord { record }
                if record.opcode() == 6 && record.frame_stamp() == frame)
        }));
    }
}

struct Boundary {
    live: RefCell<LiveCommandSession>,
    checksums: RefCell<FrameChecksumState>,
    now: Cell<u32>,
    process_millis: Option<u32>,
    sampled_frame: Cell<Option<u32>>,
    applied: RefCell<Vec<(u32, CommandRecord, bool)>>,
}

impl Boundary {
    fn new(live: LiveCommandSession) -> Self {
        Self {
            live: RefCell::new(live),
            checksums: RefCell::new(FrameChecksumState::default()),
            now: Cell::new(1000),
            process_millis: None,
            sampled_frame: Cell::new(None),
            applied: RefCell::new(Vec::new()),
        }
    }
}

impl NetworkCommandBoundary for Boundary {
    fn exchange_commands(
        &self,
        context: NetworkQueueContext,
        queues: &mut NetworkCommandQueues<'_>,
    ) -> Result<Option<NetworkQueuePass>, NetworkCommandError> {
        let mut live = self.live.borrow_mut();
        if self.sampled_frame.replace(Some(context.frame)) != Some(context.frame)
            && let Some(milliseconds) = self.process_millis
        {
            live.record_processing_time(milliseconds);
        }
        let ahead = queues.max_ahead_frames().unwrap_or(10);
        live.poll(
            queues,
            context,
            ahead,
            self.now.get(),
            (self.now.get()).wrapping_mul(16),
            0,
            0,
        )
        .map(Some)
        .map_err(|error| NetworkCommandError::Unavailable(error.to_string()))
    }

    fn record_frame_checksum(&self, frame: u32, checksum: u32) {
        self.checksums
            .borrow_mut()
            .record_frame_checksum(frame, checksum);
    }

    fn compare_frame_info(
        &self,
        record: &CommandRecord,
        current_frame: i32,
    ) -> Result<bool, NetworkCommandError> {
        self.checksums
            .borrow()
            .compare_frame_info(FrameInfo::decode(record).unwrap(), current_frame)
            .map_err(NetworkCommandError::Mismatch)
    }
}

fn stop_scene() -> (Simulation, RuleSet, u64) {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n[VehicleTypes]\n0=AMCV\n[AircraftTypes]\n[BuildingTypes]\n\
         [AMCV]\nStrength=1000\nSpeed=4\nSight=0\n\
         Locomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\nMovementZone=Normal\n\
         [Clear]\nFoot=100%\nTrack=100%\nWheel=100%\nFloat=0%\n",
    ))
    .unwrap();
    let mut sim = Simulation::with_seed(73);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    for (index, name) in ["Alpha", "Beta"].iter().enumerate() {
        let owner = sim.interner.intern(name);
        sim.houses
            .register(HouseState::new(owner, index as u8, None, true, 0, 10));
    }
    sim.session.set_game_mode(ScenarioGameMode::Lan);
    sim.session.install_network_timing(10);
    sim.session.game_options_mut_for_test().short_game = false;
    let costs = rules
        .terrain_rules
        .semantics_for_land_type(crate::rules::terrain_rules::LandType::Clear.as_index())
        .unwrap()
        .speed_costs;
    let cells = (0..64)
        .flat_map(|y| {
            (0..64).map(move |x| {
                let mut cell = crate::map::resolved_terrain::test_flat_cell(x, y);
                cell.speed_costs = costs;
                cell.base_speed_costs = costs;
                cell
            })
        })
        .collect();
    sim.install_resolved_terrain_for_new_map(
        crate::map::resolved_terrain::ResolvedTerrainGrid::from_cells(64, 64, cells),
    );
    assert!(sim.rebuild_dynamic_navigation(&rules));
    crate::sim::arena_fixture::supply_native_map(&mut sim);
    let actor = sim
        .spawn_object("AMCV", "Alpha", 20, 20, 0, &rules)
        .unwrap();
    sim.spawn_object("AMCV", "Beta", 25, 25, 0, &rules).unwrap();
    (sim, rules, actor)
}

fn production_frame(sim: &mut Simulation, rules: &RuleSet, boundary: &Boundary) -> SimFrameOutput {
    let applied = |sim: &Simulation, record: &CommandRecord, applied: bool| {
        boundary
            .applied
            .borrow_mut()
            .push((sim.session().binary_frame(), record.clone(), applied));
    };
    let effects = FrameEffects::empty()
        .with_network_commands(boundary)
        .with_command_applied(&applied);
    if sim.pending_network_wait() {
        sim.resume_network_wait(rules, None, effects).unwrap()
    } else {
        sim.advance_app_frame(&[], rules, None, 33, TickLane::Ordinary, effects)
            .unwrap()
    }
}

#[test]
fn real_udp_timing_commands_apply_precalc_then_adapt_to_the_slowest_peer() {
    let (mut a_live, mut b_live) = pair(73);
    a_live.mode = LiveCommandMode::Wol;
    b_live.mode = LiveCommandMode::Wol;
    // The host is House1, not an assumed zero; no second event may follow
    // the one-shot startup value until periodic processing feedback is ready.
    b_live.configure_precalculated_timing(20, 45);
    let mut a_boundary = Boundary::new(a_live);
    let mut b_boundary = Boundary::new(b_live);
    a_boundary.process_millis = Some(20);
    b_boundary.process_millis = Some(50);
    let (mut a, rules, _) = stop_scene();
    let (mut b, _, _) = stop_scene();
    for sim in [&mut a, &mut b] {
        sim.session.set_game_mode(ScenarioGameMode::Wol);
        sim.session
            .install_network_command_timing(1, std::num::NonZeroU8::new(5).unwrap(), [0, 1]);
    }
    for _ in 0..5 {
        b.issue_local_command(CommandEnvelope::synchronized(
            crate::sim::command::GameSpeedRecord {
                house_id: 1,
                frame: 0,
                speed: 3,
            }
            .encode()
            .unwrap(),
            0,
        ));
    }
    for frame in 0..=300 {
        let start = Instant::now();
        let mut completed = [false; 2];
        let mut checksums = [None; 2];
        while !completed.iter().all(|done| *done) {
            for (index, (sim, boundary)) in [(&mut a, &a_boundary), (&mut b, &b_boundary)]
                .into_iter()
                .enumerate()
            {
                if completed[index] {
                    continue;
                }
                boundary
                    .now
                    .set(1000 + frame * 10 + (start.elapsed().as_millis() / 16) as u32);
                let output = production_frame(sim, &rules, boundary);
                assert!(output.network_failure.is_none());
                if let Some(sync) = output.multiplayer_checksum {
                    checksums[index] = Some(sync.checksum.unwrap().value);
                }
                completed[index] = output.tick.frame_committed;
            }
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "timing frame {frame} stalled"
            );
            std::thread::yield_now();
        }
        assert_eq!(checksums[0], checksums[1], "timing frame {frame} CRC");
        if frame == 10 {
            assert_eq!(
                a.session
                    .network_command_timing()
                    .unwrap()
                    .frames_per_second(),
                45
            );
            assert_eq!(
                b.session.network_command_timing(),
                a.session.network_command_timing()
            );
        }
    }
    for sim in [&a, &b] {
        let timing = sim.session.network_command_timing().unwrap();
        assert_eq!(timing.maximum_processing_millis([0, 1]), Some(50));
        assert_eq!(timing.frames_per_second(), 20);
        assert_eq!(timing.frame_send_rate().get(), 5);
    }
    assert_eq!(
        a.session.network_command_timing(),
        b.session.network_command_timing()
    );
}

#[test]
fn three_udp_clients_keep_adaptive_timing_after_the_original_publisher_exits() {
    let mut transports: [LanUdpTransport; 3] = std::array::from_fn(|_| transport());
    let addresses: [SocketAddrV4; 3] =
        std::array::from_fn(|index| transports[index].primary_addr().unwrap());
    for (index, transport) in transports.iter_mut().enumerate() {
        for (peer, address) in addresses.iter().enumerate() {
            if peer != index {
                transport
                    .connections_mut()
                    .add_peer(peer as i32, *address)
                    .unwrap();
            }
        }
    }
    let mut boundaries: Vec<_> = transports
        .into_iter()
        .enumerate()
        .map(|(index, transport)| {
            let live = LiveCommandSession::new(
                transport,
                index as i8,
                73,
                NonZeroU32::new(5).unwrap(),
                LiveCommandMode::Lan,
            )
            .unwrap();
            let mut boundary = Boundary::new(live);
            boundary.process_millis = Some(if index == 2 { 50 } else { 20 });
            boundary
        })
        .collect();
    let (first, rules, _) = stop_scene();
    let mut worlds = [first, stop_scene().0, stop_scene().0];
    for (index, sim) in worlds.iter_mut().enumerate() {
        let third = sim.interner.intern("Gamma");
        sim.houses
            .register(HouseState::new(third, 2, None, true, 0, 10));
        sim.spawn_object("AMCV", "Gamma", 30, 30, 0, &rules)
            .unwrap();
        sim.session
            .set_current_house_for_test(Some(sim.houses.order()[index]));
        sim.session.install_network_command_timing(
            0,
            std::num::NonZeroU8::new(5).unwrap(),
            [0, 1, 2],
        );
    }
    // Admit enough ordinary synchronized records to reach native's adaptive
    // gate. EXIT itself then crosses all three real reliable UDP connections.
    for _ in 0..5 {
        assert!(
            worlds[1].issue_local_command(CommandEnvelope::synchronized(
                crate::sim::command::GameSpeedRecord {
                    house_id: 1,
                    frame: 0,
                    speed: 3
                }
                .encode()
                .unwrap(),
                0,
            ))
        );
    }
    assert!(
        worlds[0].issue_local_command(CommandEnvelope::synchronized(
            crate::sim::command::ExitRecord {
                house_id: 0,
                frame: 0
            }
            .encode()
            .unwrap(),
            0,
        ))
    );
    let mut retired = false;
    for frame in 0..=300 {
        let start = Instant::now();
        let mut completed = [retired, false, false];
        let mut checksums = [None; 3];
        while !completed.iter().all(|done| *done) {
            for index in 0..3 {
                if completed[index] {
                    continue;
                }
                boundaries[index]
                    .now
                    .set(1000 + frame * 10 + (start.elapsed().as_millis() / 16) as u32);
                let output = production_frame(&mut worlds[index], &rules, &boundaries[index]);
                assert!(output.network_failure.is_none());
                if let Some(sync) = output.multiplayer_checksum {
                    checksums[index] = Some(sync.checksum.unwrap().value);
                }
                // Local EXIT4C7917 skips MainTick55DE7E's frame commit.
                // Consume the same terminal edge as the app instead of running
                // that exited world's Logic again on its unchanged frame10.
                if let Some(owner) = worlds[index].take_executed_exit_owner() {
                    assert_eq!(index, 0, "only the original publisher leaves");
                    assert_eq!(owner, worlds[index].houses.order()[0]);
                    assert_eq!(frame, 10, "EXIT keeps its admitted execution frame");
                    assert!(worlds[index].quit_requested());
                    assert!(!output.tick.frame_committed);
                    completed[index] = true;
                } else {
                    completed[index] = output.tick.frame_committed;
                }
            }
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "host-loss timing frame{frame} stalled"
            );
            std::thread::yield_now();
        }
        assert_eq!(checksums[1], checksums[2], "surviving frame{frame} CRC");
        if !retired
            && worlds
                .iter()
                .all(|sim| sim.houses[&sim.houses.order()[0]].left_game())
        {
            for boundary in &mut boundaries[1..] {
                assert!(boundary.live.borrow_mut().remove_peer(0));
                assert_eq!(boundary.live.borrow().peers().len(), 1);
            }
            retired = true;
        }
    }
    assert!(
        retired,
        "the transmitted EXIT never executed on every client"
    );
    for sim in &mut worlds[1..] {
        assert_eq!(sim.network_command_queues().timing_host_house(), Some(1));
        let timing = sim.session.network_command_timing().unwrap();
        assert_eq!(timing.maximum_processing_millis([1, 2]), Some(50));
        assert_eq!(timing.frames_per_second(), 20);
    }
    assert_eq!(
        worlds[1].session.network_command_timing(),
        worlds[2].session.network_command_timing()
    );
    assert_eq!(worlds[1].state_hash(), worlds[2].state_hash());
}

#[test]
fn two_udp_clients_execute_the_delayed_stop_through_the_production_frame_boundary() {
    let (a_live, b_live) = pair(73);
    let a_boundary = Boundary::new(a_live);
    let b_boundary = Boundary::new(b_live);
    let (mut a, rules, actor) = stop_scene();
    let (mut b, _, second_actor) = stop_scene();
    assert_eq!(actor, second_actor);
    let owner = a.interner.get("Alpha").unwrap();
    let command = a.encode_stop_record(owner, actor).unwrap();
    assert!(a.issue_local_command(CommandEnvelope::synchronized(command, 0)));
    let mut executed = [0; 2];
    for frame in 0..=10 {
        if frame == 10 {
            for sim in [&mut a, &mut b] {
                sim.substrate
                    .entities
                    .get_mut(actor)
                    .unwrap()
                    .navigation
                    .set_nav_list_for_test(
                        vec![crate::sim::components::NavTargetRef::cell(21, 20)],
                        false,
                    );
            }
        }
        let start = Instant::now();
        let mut completed = [false; 2];
        let mut checksums = [None; 2];
        while !completed.iter().all(|done| *done) {
            for (index, (sim, boundary)) in [(&mut a, &a_boundary), (&mut b, &b_boundary)]
                .into_iter()
                .enumerate()
            {
                if completed[index] {
                    continue;
                }
                boundary
                    .now
                    .set(1000 + frame * 100 + (start.elapsed().as_millis() / 16) as u32);
                let output = production_frame(sim, &rules, boundary);
                if let Some(sync) = output.multiplayer_checksum {
                    checksums[index] = Some(sync.checksum.unwrap().value);
                }
                if output.tick.frame_committed {
                    completed[index] = true;
                    executed[index] += output.tick.executed_commands;
                }
            }
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "frame{frame} stalled"
            );
            std::thread::yield_now();
        }
        assert_eq!(checksums[0], checksums[1], "frame{frame} CRC");
        assert_eq!(executed, if frame < 10 { [0, 0] } else { [1, 1] });
    }
    for sim in [&a, &b] {
        assert!(
            sim.substrate
                .entities
                .get(actor)
                .unwrap()
                .navigation
                .nav_list()
                .is_empty()
        );
        assert_eq!(sim.session.binary_frame(), 11);
    }
}

#[test]
fn real_udp_mass_order_survives_backpressure_and_executes_one_shared_frame() {
    use crate::sim::command::{Command, MegaMissionRecord, StopRecord};
    use std::collections::{BTreeMap, BTreeSet};

    let (a_live, b_live) = pair(73);
    let a_boundary = Boundary::new(a_live);
    let b_boundary = Boundary::new(b_live);
    let (mut a, rules, first) = stop_scene();
    let (mut b, _, _) = stop_scene();
    let mut actors = vec![first];
    // The existing Alpha actor already occupies (20,20).
    let cells = (4..24)
        .flat_map(|y| (4..34).map(move |x| (x, y)))
        .filter(|cell| *cell != (20, 20));
    for (x, y) in cells {
        let left = a.spawn_object("AMCV", "Alpha", x, y, 0, &rules).unwrap();
        assert_eq!(
            left,
            b.spawn_object("AMCV", "Alpha", x, y, 0, &rules).unwrap()
        );
        actors.push(left);
    }
    assert_eq!(actors.len(), 600);
    let owner = a.interner.get("Alpha").unwrap();
    a.session.set_current_house_for_test(Some(owner));
    b.session.set_current_house_for_test(b.interner.get("Beta"));
    let moves = actors
        .iter()
        .map(|&entity_id| {
            a.encode_megamission_record(
                owner,
                &Command::Move {
                    entity_id,
                    target_rx: 45,
                    target_ry: 45,
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(
        a.issue_local_commands(
            (0..20_000).map(|index| {
                CommandEnvelope::synchronized(moves[index % moves.len()].clone(), 0)
            })
        )
    );
    assert_eq!(a.pending_local_command_count(), 20_000);
    assert!(
        !a.issue_local_commands(
            (0..20_000).map(|index| {
                CommandEnvelope::synchronized(moves[index % moves.len()].clone(), 0)
            })
        )
    );
    assert_eq!(
        a.pending_local_command_count(),
        20_000,
        "rejected batch is atomic"
    );

    let started = Instant::now();
    let mut checksums = [BTreeMap::new(), BTreeMap::new()];
    let mut saw_backpressure = false;
    // Advance peers independently: the peer that already committed frame zero
    // must still service its socket while the sender drains more than 32 packets.
    for until in [11, 26] {
        while a.session().binary_frame() < until || b.session().binary_frame() < until {
            for (index, (sim, boundary)) in [(&mut a, &a_boundary), (&mut b, &b_boundary)]
                .into_iter()
                .enumerate()
            {
                if sim.session().binary_frame() >= until {
                    continue;
                }
                boundary
                    .now
                    .set(1000 + (started.elapsed().as_millis() / 16) as u32);
                let frame = sim.session().binary_frame();
                let output = production_frame(sim, &rules, boundary);
                assert!(output.network_failure.is_none());
                if let Some(sync) = output.multiplayer_checksum {
                    checksums[index].insert(frame, sync.checksum.unwrap().value);
                }
                if index == 0
                    && sim.pending_local_command_count() != 0
                    && sim.pending_network_wait()
                {
                    saw_backpressure = true;
                }
            }
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "mass UDP exchange stalled"
            );
            std::thread::yield_now();
        }
        if until == 11 {
            for boundary in [&a_boundary, &b_boundary] {
                let applied = boundary.applied.borrow();
                let moves = applied
                    .iter()
                    .filter_map(|(frame, record, ok)| {
                        MegaMissionRecord::decode(record).map(|command| (*frame, command, *ok))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(moves.len(), 20_000);
                assert!(moves.iter().all(|(frame, _, ok)| *frame == 10 && *ok));
                assert_eq!(
                    moves
                        .iter()
                        .map(|(_, command, _)| command.source_id as u64)
                        .collect::<BTreeSet<_>>(),
                    actors.iter().copied().collect()
                );
            }
            let tick = a.session().tick();
            let stops = actors
                .iter()
                .map(|&actor| {
                    CommandEnvelope::synchronized(a.encode_stop_record(owner, actor).unwrap(), tick)
                })
                .collect::<Vec<_>>();
            assert!(a.issue_local_commands(stops));
        }
    }
    assert!(
        saw_backpressure,
        "fixture must cross the live 32-packet send queue"
    );
    assert_eq!(checksums[0], checksums[1]);
    for (sim, boundary) in [(&a, &a_boundary), (&b, &b_boundary)] {
        let applied = boundary.applied.borrow();
        let stops = applied
            .iter()
            .filter_map(|(_, record, ok)| {
                StopRecord::decode(record).map(|command| (command.entity_id as u64, *ok))
            })
            .collect::<Vec<_>>();
        assert_eq!(stops.len(), actors.len());
        assert!(stops.iter().all(|(_, ok)| *ok));
        assert_eq!(
            stops.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>(),
            actors.iter().copied().collect()
        );
        assert!(actors.iter().all(|id| {
            sim.substrate
                .entities
                .get(*id)
                .unwrap()
                .navigation
                .nav_list()
                .is_empty()
        }));
    }
}
