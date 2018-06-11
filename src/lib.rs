//
// Rust version of Firewall camkes component
// see `firewall.c` for the original C version
// Original source: https://github.com/seL4/camkes-vm/tree/master/components/Firewall
//
#![feature(libc)]

#[macro_use]
extern crate lazy_static;

extern crate libc;
extern crate smoltcp;
extern crate spin;

mod constants;
mod externs;
mod utils;

/// transmit `len` bytes from `client_buf` to `ethdriver_buf`
/// returns number of transmitted bytes
/// int client_tx(int len)
/// returns -1 if the ethernet driver fails, 0 otherwise
#[no_mangle]
pub extern "C" fn client_tx(len: i32) -> i32 {
    let mut ret = utils::RET_CLIENT_TX.lock();
    let eth_packet = utils::fetch_client_data(len as usize);

    // process frame
    match utils::process_ethernet(
        eth_packet,
        utils::PACKETS_TX.clone(),
        utils::FRAGMENTS_TX.clone(),
        utils::FN_PACKET_OUT.clone(),
        false, // no need to check MAC
    ) {
        Ok(_) => {}
        Err(_e) => {
            #[cfg(feature = "debug-print")]
            println_sel4(format!(
                "Firewall client_tx: error processing eth_packet: {}",
                _e
            ));
        }
    }

    // send 0 to N packets
    {
        *ret = 0;
        let mut packets = utils::PACKETS_TX.lock();
        while !packets.is_empty() {
            let eth_packet = packets.remove(0);
            if utils::dispatch_data_to_ethdriver(eth_packet) == -1 {
                // If enqueue fails, return immediately
                *ret = -1;
                break;
            }
        }
    }
    *ret // will do  a bitwise copy
}

/// copy `len` data from `ethdriver_buf` into `client_buf`
/// return 0 if data are received, 1 if more data are in the buffer and `client_rx()`
/// should be called again, -1 if no data are received (either the packet was dropped,
/// or `clien_rx` was called without any data being available)
#[no_mangle]
pub extern "C" fn client_rx(len: *mut i32) -> i32 {
    let mut ret = utils::RET_CLIENT_RX.lock();
    loop {
        match utils::fetch_data_from_ethdriver() {
            utils::EthdriverRxStatus::NoData => {
                break;
            }
            utils::EthdriverRxStatus::Data(eth_packet) => {
                // process eth_packet, possibly enqueue to PACKETS_RX and break
                match utils::process_ethernet(
                    eth_packet,
                    utils::PACKETS_RX.clone(),
                    utils::FRAGMENTS_RX.clone(),
                    utils::FN_PACKET_IN.clone(),
                    true, // check the MAC address
                ) {
                    Ok(_) => {}
                    Err(_e) => {
                        #[cfg(feature = "debug-print")]
                        println_sel4(format!(
                            "Firewall client_rx: error processing Data(eth_packet): {}",
                            _e
                        ));
                    }
                }
                break;
            }
            utils::EthdriverRxStatus::MoreData(eth_packet) => {
                // process eth_packet, possibly enqueue to PACKETS_RX and check for more data
                match utils::process_ethernet(
                    eth_packet,
                    utils::PACKETS_RX.clone(),
                    utils::FRAGMENTS_RX.clone(),
                    utils::FN_PACKET_IN.clone(),
                    true, // check the MAC address
                ) {
                    Ok(_) => {}
                    Err(_e) => {
                        #[cfg(feature = "debug-print")]
                        println_sel4(format!(
                            "Firewall client_rx: error processing MoreData(eth_packet): {}",
                            _e
                        ));
                    }
                }
            }
        };
    }

    {
        let mut packets = utils::PACKETS_RX.lock();
        *ret = match packets.is_empty() {
            true => -1,
            false => {
                // enqueue a single packet
                let eth_packet = packets.remove(0);
                let data_len = utils::copy_data_to_client_buf(eth_packet);
                unsafe {
                    *len = data_len;
                }
                if packets.is_empty() {
                    0 // No more data
                } else {
                    1 // More data
                }
            }
        };
        *ret // will do  a bitwise copy
    }
}

/// Ethdriver RX calls has_data_callback when new packet(s) is available
/// Pass through to the VM to eliminate this Camkes thread.
/// We are assuming there is only one client connected to the firewall
#[no_mangle]
pub extern "C" fn ethdriver_has_data_callback(_badge: u32) {
    #[cfg(feature = "debug-print")]
    println_sel4(format!(
        "Firewall ethdriver_has_data_callback: got badge = {}, calling client_emit(1);",
        _badge
    ));
    unsafe {
        externs::client_emit(1);
    }
}

/// get eth device's MAC address
/// void client_mac(uint8_t *b1, uint8_t *b2, uint8_t *b3, uint8_t *b4, uint8_t *b5, uint8_t *b6)
#[no_mangle]
pub extern "C" fn client_mac(
    b1: &mut u8,
    b2: &mut u8,
    b3: &mut u8,
    b4: &mut u8,
    b5: &mut u8,
    b6: &mut u8,
) {
    unsafe {
        externs::ethdriver_mac(b1, b2, b3, b4, b5, b6);
    }
}
