#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use capycoding_esp::ble::ble_task;
use capycoding_esp::wifi::wifi_task;
use capycoding_esp::{CapyState, WeactTermInitPins, ui_task};
use embassy_executor::Spawner;
use embedded_storage::{ReadStorage, Storage};
use esp_backtrace as _;
use esp_bootloader_esp_idf::partitions;
use esp_hal::clock::CpuClock;
use esp_hal::spi::{self, master::Spi};
use esp_hal::{time::Rate, timer::timg::TimerGroup};
use esp_storage::FlashStorage;

use log::info;
use static_cell::StaticCell;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static RADIO: StaticCell<esp_radio::Controller<'static>> = StaticCell::new();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> () {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // shit ton of memory alloc
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 66320);
    esp_alloc::heap_allocator!(size: 200 * 1024);

    // rtos init
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");

    // init radio
    let radio_init: esp_radio::Controller<'_> =
        esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");

    let radio = RADIO.init(radio_init);

    // init for term
    let mosi_pin = peripherals.GPIO5;
    let sclk_pin = peripherals.GPIO4;

    let spi_bus = Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(Rate::from_khz(100))
            .with_mode(spi::Mode::_0),
    )
    .unwrap()
    .with_sck(sclk_pin)
    .with_mosi(mosi_pin);

    let term_init_pins = WeactTermInitPins {
        cs_pin: peripherals.GPIO3,
        dc_pin: peripherals.GPIO2,
        rst_pin: peripherals.GPIO1,
        busy_pin: peripherals.GPIO0,
    };

    // spawn tasks
    spawner.spawn(ble_task(radio, peripherals.BT)).unwrap();
    spawner.spawn(wifi_task(radio, peripherals.WIFI)).unwrap();
    spawner.spawn(ui_task(spi_bus, term_init_pins)).unwrap();

    let mut flash = FlashStorage::new(peripherals.FLASH);

    let mut state = CapyState::load(&mut flash);
    state.wifi_credentials.ssid = "lol".try_into().unwrap();
    state.wifi_credentials.password = "lol123".try_into().unwrap();
    state.api_tokens.github = "abcdefg".try_into().unwrap();
    state.write(&mut flash);

    info!("State: {state:?}");

    // let mut pt_mem = [0u8; partitions::PARTITION_TABLE_MAX_LEN];

    // let pt = partitions::read_partition_table(&mut flash, &mut pt_mem).unwrap();

    // for i in 0..pt.len() {
    //     let raw = pt.get_partition(i).unwrap();
    //     info!("pt i:{i}, value: {raw:?}");
    // }

    // let nvs = pt
    //     .find_partition(partitions::PartitionType::Data(
    //         partitions::DataPartitionSubType::Nvs,
    //     ))
    //     .unwrap()
    //     .unwrap();

    // let mut nvs_partition = nvs.as_embedded_storage(&mut flash);

    // let mut bytes = [0u8; 32];
    // info!("NVS partition size = {}", nvs_partition.capacity());

    // let offset_in_nvs_partition = 0;

    // nvs_partition
    //     .read(offset_in_nvs_partition, &mut bytes)
    //     .unwrap();
    // info!(
    //     "Read from {:x}:  {:02x?}",
    //     offset_in_nvs_partition,
    //     &bytes[..32]
    // );

    // bytes[0x00] = bytes[0x00].wrapping_add(1);
    // bytes[0x01] = bytes[0x01].wrapping_add(2);
    // bytes[0x02] = bytes[0x02].wrapping_add(3);
    // bytes[0x03] = bytes[0x03].wrapping_add(4);
    // bytes[0x04] = bytes[0x04].wrapping_add(1);
    // bytes[0x05] = bytes[0x05].wrapping_add(2);
    // bytes[0x06] = bytes[0x06].wrapping_add(3);
    // bytes[0x07] = bytes[0x07].wrapping_add(4);

    // nvs_partition
    //     .write(offset_in_nvs_partition, &bytes)
    //     .unwrap();
    // info!(
    //     "Written to {:x}: {:02x?}",
    //     offset_in_nvs_partition,
    //     &bytes[..32]
    // );

    // let mut reread_bytes = [0u8; 32];
    // nvs_partition.read(0, &mut reread_bytes).unwrap();
    // info!(
    //     "Read from {:x}:  {:02x?}",
    //     offset_in_nvs_partition,
    //     &reread_bytes[..32]
    // );

    // info!("Reset (CTRL-R in espflash) to re-read the persisted data.");

    // loop {}
}
