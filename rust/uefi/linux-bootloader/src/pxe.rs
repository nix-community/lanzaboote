use core::ffi::c_void;

use uefi::{proto::unsafe_protocol, Status};

/// PXE support

const PXEBASE_CODE_PROTOCOL_REVISION: u64 = 0x00010000;
const PXEBASE_CODE_MAX_ARP_ENTRIES: usize = 8;
const PXEBASE_CODE_MAX_ROUTE_ENTRIES: usize = 8;
const PXEBASE_CODE_MAX_IPCNT: usize = 8;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct IPv4Address {
    addr: [u8; 4]
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct IPv6Address {
    addr: [u8; 16]
}

#[repr(C)]
union IPAddress {
    addr: [u8; 4],
    v4: IPv4Address,
    v6: IPv6Address
}

#[derive(Debug)]
#[repr(C)]
struct MACAddress {
    addr: [u8; 32]
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct PXEBaseCodeDHCPv6Packet {
    message_type: u32,
    transaction_id: u32,
    dhcp_options: [u8; 1024]
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct PXEBaseCodeDHCPv4Packet {
    bootp_opcode: u8,
    bootp_hw_type: u8,
    bootp_hw_addr_len: u8,
    bootp_gate_hops: u8,
    bootp_ident: u32,
    bootp_seconds: u16,
    bootp_flags: u16,
    bootp_client_ip_addr: [u8; 4],
    bootp_yi_addr: [u8; 4],
    bootp_si_addr: [u8; 4],
    bootp_gi_addr: [u8; 4],
    bootp_hw_addr: [u8; 16],
    bootp_srv_name: [u8; 64],
    bootp_bootfile: [u8; 128],
    dhcp_magik: u32,
    dhcp_options: [u8; 56]
}

#[repr(C)]
union PXEBaseCodePacket {
    raw: [u8; 1472],
    dhcpv4: PXEBaseCodeDHCPv4Packet,
    dhcpv6: PXEBaseCodeDHCPv6Packet
}

#[derive(Debug)]
#[repr(C)]
struct PXEBaseCodeTFTPError {
    error_code: u8,
    error_string: [u8; 127]
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
struct PXEBaseICMPEcho {
    identifier: u16,
    sequence: u16
}

#[repr(C)]
union PXEBaseCodeICMPErrorMetadata {
    _reserved: u32,
    mtu: u32,
    pointer: u32,
    echo: PXEBaseICMPEcho
}

#[repr(C)]
struct PXEBaseCodeICMPError {
    r#type: u8,
    code: u8,
    checksum: u16,
    metadata: PXEBaseCodeICMPErrorMetadata,
    data: [u8; 494]
}

#[repr(C)]
struct PXEBaseCodeIPFilter {
    filters: u8,
    ip_count: u8,
    _reserved: u16,
    ip_list: [IPAddress; PXEBASE_CODE_MAX_IPCNT]
}

#[repr(C)]
struct PXEBaseCodeARPEntry {
    ip_addr: IPAddress,
    mac_addr: MACAddress
}

#[repr(C)]
struct PXEBaseCodeRouteEntry {
    ip_addr: IPAddress,
    subnet_mask: IPAddress,
    gateway_addr: IPAddress
}

#[repr(C)]
struct PXEBaseCodeMode {
    started: bool,
    ipv6_available: bool,
    ipv6_supported: bool,
    using_ipv6: bool,
    bis_supported: bool,
    bis_detected: bool,
    auto_arp: bool,
    send_guid: bool,
    dhcp_discover_valid: bool,
    dhcp_ack_received: bool,
    proxy_offer_received: bool,
    pxe_discover_valid: bool,
    pxe_reply_valid: bool,
    pxe_bis_reply_received: bool,
    icmp_error_received: bool,
    tftp_error_received: bool,
    make_callbacks: bool,
    ttl: u8,
    tos: u8,
    // EFI_IP_ADDRESS
    station_ip: IPAddress,
    subnet_mask: IPAddress,
    // EFI_PXE_BASE_CODE_PACKET
    dhcp_discover: PXEBaseCodePacket,
    dhcp_ack: PXEBaseCodePacket,
    proxy_offer: PXEBaseCodePacket,
    pxe_discover: PXEBaseCodePacket,
    pxe_reply: PXEBaseCodePacket,
    pxe_bis_reply: PXEBaseCodePacket,
    // IP_FILTER
    ip_filter: PXEBaseCodeIPFilter,
    arp_cache_entries: u32,
    // ARP_ENTRY array
    arp_cache: [PXEBaseCodeARPEntry; PXEBASE_CODE_MAX_ARP_ENTRIES],
    route_table_entries: u32,
    route_table: [PXEBaseCodeRouteEntry; PXEBASE_CODE_MAX_ROUTE_ENTRIES],
    // _ERROR
    icmp_error: PXEBaseCodeICMPError,
    tftp_error: PXEBaseCodeTFTPError,
}

struct PXEBaseCodeServerList {
    r#type: u16,
    accept_any_response: bool,
    _reserved: u8,
    ip_address: IPAddress
}

struct PXEBaseCodeDiscoverInfo {
    use_multicast: bool,
    use_broadcast: bool,
    use_unicast: bool,
    must_use_list: bool,
    server_multicast_ip: IPAddress,
    ip_count: u16,
    // It is a dynamically sized array based on `ip_count`…
    server_list: *mut PXEBaseCodeServerList,
}

enum PXEBaseCodeTFTPOpcode {
    First,
    GetFileSize,
    ReadFile,
    WriteFile,
    ReadDirectory,
    MulticastGetFileSize,
    MulticastReadFile,
    MulticastReadDirectory,
    Last
}

struct PXEBaseCodeMTFTPInfo {
    multicast_ip: IPAddress,
    client_port: u16,
    server_port: u16,
    listen_timeout: u16,
    transmit_timeout: u16
}

#[derive(Debug)]
#[repr(C)]
#[unsafe_protocol("03c4e603-ac28-11d3-9a2d-0090273fc14d")]
pub struct PXEBaseCodeProtocol {
    revision: u64,
    start: unsafe extern "efiapi" fn(this: &mut PXEBaseCodeProtocol, use_ipv6: bool) -> Status,
    stop: unsafe extern "efiapi" fn(this: &mut PXEBaseCodeProtocol) -> Status,
    perform_dhcp: unsafe extern "efiapi" fn(this: &mut PXEBaseCodeProtocol, sort_offers: bool) -> Status,
    discover: unsafe extern "efiapi" fn(this: &mut PXEBaseCodeProtocol, r#type: u16, layer: *mut u16, use_boot_integrity_services: bool, info: *const PXEBaseCodeDiscoverInfo),
    perform_mtftp: unsafe extern "efiapi" fn(this: &mut PXEBaseCodeProtocol,
        operation: PXEBaseCodeProtocol,
        buffer: *mut c_void,
        overwrite_file: bool,
        buffer_size: *mut usize,
        block_size: *const usize,
        server_ip: *const IPAddress,
        filename: *const u8,
        info: *const PXEBaseCodeMTFTPInfo,
        dont_use_buffer: bool),
    udp_write: unsafe extern "efiapi" fn(),
    udp_read: unsafe extern "efiapi" fn(),
    set_ip_filter: unsafe extern "efiapi" fn(),
    perform_arp: unsafe extern "efiapi" fn(),
    set_parameters: unsafe extern "efiapi" fn(),
    set_station_ip: unsafe extern "efiapi" fn(),
    set_packets: unsafe extern "efiapi" fn(),
    mode: *const PXEBaseCodeMode
}
