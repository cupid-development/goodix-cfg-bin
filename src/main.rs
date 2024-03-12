#![allow(arithmetic_overflow)]
use serde::Serialize;
use serde_with::serde_as;
use std::{
    collections::HashMap,
    env, fs,
    mem::{size_of, transmute, MaybeUninit},
    ptr::copy_nonoverlapping,
};

const TS_BIN_VERSION_START_INDEX: usize = 5;
const TS_BIN_VERSION_LEN: usize = 4;
const TS_CFG_BIN_HEAD_RESERVED_LEN: usize = 6;
const TS_CFG_OFFSET_LEN: usize = 2;
const TS_IC_TYPE_NAME_MAX_LEN: usize = 15;
const TS_CFG_BIN_HEAD_LEN: usize = size_of::<GoodixCfgBinHead>() + TS_CFG_BIN_HEAD_RESERVED_LEN;
const TS_PKG_CONST_INFO_LEN: usize = size_of::<GoodixCfgPkgConstInfo>();
const TS_PKG_REG_INFO_LEN: usize = size_of::<GoodixCfgPkgRegInfo>();
const TS_PKG_HEAD_LEN: usize = TS_PKG_CONST_INFO_LEN + TS_PKG_REG_INFO_LEN;

const TS_CFG_BLOCK_PID_LEN: usize = 8;
const TS_CFG_BLOCK_VID_LEN: usize = 8;
const TS_CFG_BLOCK_FW_MASK_LEN: usize = 9;
const TS_CFG_BLOCK_FW_PATCH_LEN: usize = 4;
const TS_CFG_BLOCK_RESERVED_LEN: usize = 9;

const GOODIX_CFG_MAX_SIZE: usize = 4096;

#[derive(Debug, Copy, Clone, Serialize)]
#[repr(C, packed(1))]
struct GoodixCfgPkgReg {
    addr: u16,
    reserved1: u8,
    reserved2: u8,
}

#[derive(Debug, Serialize)]
#[repr(C, packed(1))]
struct GoodixCfgPkgConstInfo {
    pkg_len: u32,
    ic_type: [u8; TS_IC_TYPE_NAME_MAX_LEN],
    cfg_type: u8,
    sensor_id: u8,
    hw_pid: [u8; TS_CFG_BLOCK_PID_LEN],
    hw_vid: [u8; TS_CFG_BLOCK_VID_LEN],
    fw_mask: [u8; TS_CFG_BLOCK_FW_MASK_LEN],
    fw_patch: [u8; TS_CFG_BLOCK_FW_PATCH_LEN],
    x_res_offset: u16,
    y_res_offset: u16,
    trigger_offset: u16,
}

#[derive(Debug, Serialize)]
#[repr(C, packed(1))]
struct GoodixCfgPkgRegInfo {
    cfg_send_flag: GoodixCfgPkgReg,
    version_base: GoodixCfgPkgReg,
    pid: GoodixCfgPkgReg,
    vid: GoodixCfgPkgReg,
    sensor_id: GoodixCfgPkgReg,
    fw_mask: GoodixCfgPkgReg,
    fw_status: GoodixCfgPkgReg,
    cfg_addr: GoodixCfgPkgReg,
    esd: GoodixCfgPkgReg,
    command: GoodixCfgPkgReg,
    coor: GoodixCfgPkgReg,
    gesture: GoodixCfgPkgReg,
    fw_request: GoodixCfgPkgReg,
    proximity: GoodixCfgPkgReg,
    reserved: [u8; TS_CFG_BLOCK_RESERVED_LEN],
}

#[derive(Debug, Serialize)]
#[repr(C, packed(1))]
struct GoodixCfgBinHead {
    bin_len: u32,
    checksum: u8,
    bin_version: [u8; TS_BIN_VERSION_LEN],
    pkg_num: u8,
}

#[derive(Debug, Serialize)]
#[repr(C)]
struct GoodixCfgPackage {
    cnst_info: GoodixCfgPkgConstInfo,
    reg_info: GoodixCfgPkgRegInfo,
    #[serde(skip)]
    cfg: *const u8,
    pkg_len: u32,
}

#[derive(Debug, Serialize)]
#[repr(C)]
struct GoodixCfgBin {
    head: GoodixCfgBinHead,
    cfg_pkgs: Vec<GoodixCfgPackage>,
    ic_configs: HashMap<u8, GoodixIcConfig>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[repr(C)]
struct GoodixIcConfig {
    len: i32,
    #[serde_as(as = "[_; 4096]")]
    data: [u8; GOODIX_CFG_MAX_SIZE],
}

#[derive(Debug)]
enum Error {
    InvalidSize,
    LengthCheckFail,
    ChecksumMismatch,
    InvalidOffset,
}

impl GoodixCfgBin {
    pub fn parse(input: &[u8]) -> Result<Self, Error> {
        #[allow(invalid_value)]
        let mut this: GoodixCfgBin = unsafe { MaybeUninit::zeroed().assume_init() };

        if input.len() < size_of::<GoodixCfgBinHead>() {
            return Err(Error::InvalidSize);
        }

        unsafe {
            copy_nonoverlapping(
                input.as_ptr(),
                transmute(&mut this.head),
                size_of::<GoodixCfgBinHead>(),
            )
        };

        if input.len() as u32 != this.head.bin_len {
            return Err(Error::LengthCheckFail);
        }

        let mut checksum = 0;

        for i in TS_BIN_VERSION_START_INDEX..input.len() {
            checksum += input[i];
        }

        if checksum != this.head.checksum {
            return Err(Error::ChecksumMismatch);
        }

        this.cfg_pkgs = Vec::with_capacity(this.head.pkg_num as usize);

        let mut offset1;
        let mut offset2;
        for i in 0..this.head.pkg_num as usize {
            // This overflows intentionally???
            offset1 = input[TS_CFG_BIN_HEAD_LEN + i * TS_CFG_OFFSET_LEN]
                + (input[TS_CFG_BIN_HEAD_LEN + i * TS_CFG_OFFSET_LEN + 1] << 8);

            let mut cfg_pkg: GoodixCfgPackage = unsafe { MaybeUninit::zeroed().assume_init() };

            if i == this.head.pkg_num as usize - 1 {
                cfg_pkg.pkg_len = input.len() as u32 - offset1 as u32;
            } else {
                // This too???
                offset2 = input[TS_CFG_BIN_HEAD_LEN + i * TS_CFG_OFFSET_LEN + 2]
                    + (input[TS_CFG_BIN_HEAD_LEN + i * TS_CFG_OFFSET_LEN + 3] << 8);

                if offset2 <= offset1 {
                    return Err(Error::InvalidOffset);
                }

                cfg_pkg.pkg_len = (offset2 - offset1) as u32;
            }

            unsafe {
                copy_nonoverlapping(
                    input.as_ptr().add(offset1 as usize),
                    transmute(&mut cfg_pkg.cnst_info),
                    TS_PKG_CONST_INFO_LEN,
                );
                copy_nonoverlapping(
                    input
                        .as_ptr()
                        .add(offset1 as usize)
                        .add(TS_PKG_CONST_INFO_LEN),
                    transmute(&mut cfg_pkg.reg_info),
                    TS_PKG_REG_INFO_LEN,
                );
                cfg_pkg.cfg = &input[offset1 as usize + TS_PKG_HEAD_LEN];
            }

            // Get the ic config for this sensor ID
            let cfg_len = cfg_pkg.pkg_len as usize - TS_PKG_CONST_INFO_LEN - TS_PKG_REG_INFO_LEN;
            let mut ic_config_data = [0; GOODIX_CFG_MAX_SIZE];
            unsafe { copy_nonoverlapping(cfg_pkg.cfg, ic_config_data.as_mut_ptr(), cfg_len) }

            let ic_config = GoodixIcConfig {
                len: cfg_len as i32,
                data: ic_config_data,
            };

            this.ic_configs
                .insert(cfg_pkg.cnst_info.cfg_type, ic_config);

            this.cfg_pkgs.push(cfg_pkg);
        }

        Ok(this)
    }
}

fn main() {
    let cfg_bin_file = env::args().nth(1).expect("No cfg bin file provided");
    let contents = fs::read(cfg_bin_file).expect("Failed to read cfg bin file");
    let cfg_bin = GoodixCfgBin::parse(&contents).unwrap();

    println!("{}", serde_json::to_string_pretty(&cfg_bin).unwrap());
}
