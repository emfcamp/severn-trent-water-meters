//! This example runs on the Heltec WiFi LoRa ESP32 board, which has a builtin Semtech Sx1276 radio.
//! It demonstrates LORA P2P receive functionality in conjunction with the lora_p2p_send example.
#![no_std]
#![no_main]

use bitfields::bitfield;
use defmt::{info,error};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex};
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay,Duration,Instant, Ticker};
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig};
use esp_hal::{
    Async,
    clock::CpuClock,
    spi::{
        Mode,
        master::{Config, Spi},
    },
    rng::Rng,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_println as _;
use lora_phy::iv::GenericSx126xInterfaceVariant;
use lorawan_device::async_device::radio::Timer;
use lora_phy::sx126x;
use lora_phy::sx126x::Sx1262;
use lora_phy::sx126x::Sx126x;
use lora_phy::sx126x::TcxoCtrlVoltage;
use lora_phy::LoRa;
use lorawan_device::async_device::{Device, JoinMode, JoinResponse, region};
use lorawan_device::default_crypto::DefaultFactory;
use lorawan_device::{AppEui, AppKey, DevEui};
use lora_phy::lorawan_radio::LorawanRadio;
use static_cell::StaticCell;
use esp_hal::interrupt::software::SoftwareInterruptControl;

const LORAWAN_REGION: region::Region = region::Region::EU868;
const MAX_TX_POWER: u8 = 22;

// Bitbang water meter IF
type WsReader = Mutex<CriticalSectionRawMutex, Option<(Output<'static>,Input<'static>,[u8;200])>>;
static WS_READER: WsReader = Mutex::new(None);

static SPI_BUS: StaticCell<Mutex<CriticalSectionRawMutex, esp_hal::spi::master::Spi<'static, Async>>> =
    StaticCell::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[bitfield(u16)]
struct WsSymbol {
    #[bits(6)]
    _pad: u8,
    #[bits(1)]
    start: u8,
    #[bits(7)]
    d: u8,
    #[bits(1)]
    p: u8,
    #[bits(1)]
    stop: u8
}

struct LWTimer{
    start: Instant,
}

impl LWTimer {
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }
}

impl Default for LWTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer for LWTimer {
    fn reset(&mut self) {
        self.start = Instant::now();
    }

    async fn at(&mut self, millis: u64) {
        embassy_time::Timer::at(self.start + Duration::from_millis(millis)).await
    }

    async fn delay_ms(&mut self, millis: u64) {
        embassy_time::Timer::after_millis(millis).await
    }
}

enum SStatus {
    Wait,
    Start,
    Parity
}

#[embassy_executor::task(pool_size = 2)]
async fn check_ws(ws_reader: &'static WsReader) {
    // Runs double the actual bus speed to get two toggles per symbol
    let mut ticker = Ticker::every(Duration::from_hz(4800)); 
    let mut c = 0usize;
    let mut status = SStatus::Wait;
    let mut s = 0u8;
    let mut b = 0u8;    

    loop {
        let mut unlocked = ws_reader.lock().await;
        if let Some((clk,
                     dio,
                     rdr_buf)) = unlocked.as_mut() {
            clk.toggle();

            if clk.is_set_high() {
                match status {
                    SStatus::Wait => {
                        if dio.is_low() {
                            status = SStatus::Start;
                        }
                    },
                    SStatus::Start => {
                        // FIXME: This is very late night code and could do with refactoring
                        s |= (dio.is_high() as u8 ) << b;
                        b += 1;

                        if b == 7 {
                            rdr_buf[c] = s;
                            b = 0;
                            s = 0;
                            status = SStatus::Parity;

                        }
                    },
                    SStatus::Parity => {
                        status = SStatus::Wait;
                        c += 1;
                        if c > rdr_buf.len() - 1 {
                            if let Ok(s) = str::from_utf8(rdr_buf)
                            {
                                info!("{}",&s);
                                // FIXME: This needs tidying up
                                let mut chunks = s.split('\r').next().unwrap().split(';');
                                chunks.next().unwrap();
                                let reading: u32 = chunks.next().expect("Failed to get next reading chunk")[2..]
                                    .parse().expect("Failed to collect reading");
                                let serial = chunks.next().unwrap();
                                info!("Read from meter: \n\tReading: {:06}m³\n\tSerial: 20{}",
                                    reading, serial[2..]
                                );
                            }
                            clk.toggle();
                            break;
                        }
                    }
                }
                
            }
        }
        ticker.next().await;
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let ws_reader_buf = [0u8;200];

    // Set up ESP32
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    let timer_group = TimerGroup::new(peripherals.TIMG0);

    let software_interrupt = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timer_group.timer0, software_interrupt.software_interrupt0);

    // Initialize LoRa SPI for Driver
    let nss = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());
    let sclk = peripherals.GPIO9;
    let mosi = peripherals.GPIO10;
    let miso = peripherals.GPIO11;

    // Init LoRa IC control signals
    let reset = Output::new(peripherals.GPIO12, Level::Low, OutputConfig::default());
    let busy = Input::new(peripherals.GPIO13, InputConfig::default());
    let dio1 = Input::new(peripherals.GPIO14, InputConfig::default());

    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(Rate::from_khz(200))
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_miso(miso)
    .into_async();

    // Initialize the static SPI bus
    let spi_bus: &Mutex<_, _> = SPI_BUS.init(Mutex::new(spi));
    let spi_device = embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice::new(spi_bus, nss);

    // Create the SX126x configuration
    let sx126x_config = sx126x::Config {
        chip: Sx1262,
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: false,
        rx_boost: true,
    };

    // Create the radio instance
    let iv = GenericSx126xInterfaceVariant::new(reset, dio1, busy, None, None).unwrap();
    let lora = LoRa::new(Sx126x::new(spi_device, iv, sx126x_config), false, Delay)
        .await
        .unwrap();

    let radio: LorawanRadio<_, _, MAX_TX_POWER> = lora.into();
    let region: region::Configuration = region::Configuration::new(LORAWAN_REGION);
    let mut device: Device<_, DefaultFactory, _, _> = Device::new(region, radio, LWTimer::new(), Rng::new());

    // Init secondary SPI for meter reading
    let ws_clk = Output::new(peripherals.GPIO45, Level::High, OutputConfig::default());
    let ws_dio = Input::new(peripherals.GPIO46, InputConfig::default());
    {
        *(WS_READER.lock().await) = Some((ws_clk,ws_dio, ws_reader_buf));
    }

    defmt::info!("Joining LoRaWAN network");

    loop {
        let _wspy = spawner.spawn(check_ws(&WS_READER).expect("Failed to check ws"));
        // TODO: Adjust the EUI and Keys according to your network credentials
        if let Ok(resp) = device
            .join(&JoinMode::OTAA {
                deveui: DevEui::from([0, 0, 0, 0, 0, 0, 0, 0]),
                appeui: AppEui::from([0, 0, 0, 0, 0, 0, 0, 0]),
                appkey: AppKey::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            })
            .await {

            match resp {
                JoinResponse::JoinSuccess => {
                    info!("LoRaWAN network joined: {:?}", resp);
                    info!("Datarate is: {:?}", &device.get_datarate());
                    break;
                },
                JoinResponse::NoJoinAccept => {
                    error!("Failed to join lorawan network. No JoinAccept received. You may have set incorrect \
                           joining credentials or just not have a LoRaWAN gateway in range of your device.");
                }
            }
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    defmt::info!("Panic: {}", info);
    loop {}
}