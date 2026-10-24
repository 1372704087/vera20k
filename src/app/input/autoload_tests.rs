//! VeraX AutoLoad assignment tests: grouping, capacity, range and the
//! carrier/passenger exclusion.

use super::{PassengerFact, TransportFact, assign};

const CELL: i32 = 256;

fn passenger(id: u64, type_key: u32, size: f64, infantry: bool, pos: [i32; 2]) -> PassengerFact {
    PassengerFact {
        id,
        type_key,
        size,
        infantry,
        pos,
    }
}

fn transport(id: u64, capacity: i32, size_limit: f64, pos: [i32; 2]) -> TransportFact {
    TransportFact {
        id,
        capacity,
        size_limit,
        total: 0,
        pos,
    }
}

#[test]
fn balanced_types_fill_nearest_then_round_robin() {
    let passengers = [
        passenger(1, 1, 1.0, true, [0, 0]),
        passenger(2, 1, 1.0, true, [0, 0]),
        passenger(3, 2, 2.0, false, [0, 0]),
    ];
    let transports = [
        transport(10, 5, 2.0, [0, 0]),
        transport(11, 5, 2.0, [5 * CELL, 0]),
    ];
    assert_eq!(
        assign(&passengers, &transports),
        vec![(1, 10), (3, 11), (2, 11)]
    );
}

#[test]
fn capacity_and_size_limit_refuse_extra_passengers() {
    let passengers = [
        passenger(1, 1, 1.0, true, [0, 0]),
        passenger(2, 1, 1.0, true, [0, 0]),
        passenger(3, 2, 2.0, false, [0, 0]),
        passenger(4, 3, 4.0, false, [0, 0]),
    ];
    let transports = [transport(10, 2, 3.0, [0, 0])];
    // Two Size=1 infantrymen fill Passengers=2; the Size=2 vehicle does not
    // fit the remaining total and the Size=4 vehicle exceeds SizeLimit=3.
    assert_eq!(assign(&passengers, &transports), vec![(1, 10), (2, 10)]);
}

#[test]
fn range_boundary_and_carrier_exclusion() {
    let at_range = [30 * CELL, 0];
    let beyond_range = [31 * CELL, 0];
    let infantry = [passenger(1, 1, 1.0, true, [0, 0])];
    assert!(assign(&infantry, &[transport(10, 5, 1.0, at_range)]).len() == 1);
    assert!(assign(&infantry, &[transport(10, 5, 1.0, beyond_range)]).is_empty());

    // Vehicle 20 is both passenger and carrier: the infantryman fills it
    // first, so it never boards another hull.
    let passengers = [
        passenger(1, 1, 1.0, true, [0, 0]),
        passenger(2, 1, 1.0, true, [0, 0]),
        passenger(20, 2, 1.0, false, [0, 0]),
    ];
    let transports = [
        transport(20, 5, 2.0, [0, 0]),
        transport(30, 5, 2.0, [CELL, 0]),
    ];
    assert_eq!(assign(&passengers, &transports), vec![(1, 20), (2, 30)]);
}
