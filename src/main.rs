#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use cortex_m_rt::entry;
use sensors::dht11::{Dht11, Error, MicrosTimer};
use sht4x::{Precision, Sht4x};
use stm32f4xx_hal::{i2c::I2c, i2c::Mode, pac, prelude::*, rcc::Config};

mod sensors;

#[entry]
fn main() -> ! {
    // Получаем доступ к периферии STM32
    let dp = pac::Peripherals::take().unwrap();

    // Настраиваем тактирование: HSI, 16 МГц
    let mut rcc = dp.RCC.freeze(Config::hsi().sysclk(16.MHz()));

    // Получаем доступ к порту GPIOA
    let gpioa = dp.GPIOA.split(&mut rcc);
    // PA0 — пин к DATA DHT11
    let mut dht_pin = gpioa.pa0.into_open_drain_output();
    dht_pin.set_high();

    let gpiob = dp.GPIOB.split(&mut rcc);
    // I2C для SHT40
    let sht_i2c = I2c::new(
        dp.I2C1,
        (gpiob.pb8, gpiob.pb9),
        Mode::standard(100.kHz()),
        &mut rcc,
    );

    let mut timer = dp.TIM5.counter_us(&mut rcc);
    timer.start(u32::MAX.micros()).unwrap();

    let mut delay = dp.TIM2.delay_us(&mut rcc);

    let mut dht11 = Dht11::new(dht_pin, &timer);
    let mut sht40 = Sht4x::new(sht_i2c);

    // Основной бесконечный цикл
    loop {
        let cycle_start = timer.now_mks();

        match dht11.read() {
            Ok(reading) => defmt::info!(
                "DHT11: hum = {}%, temp = {}.{} C",
                reading.humidity,
                reading.temp_int,
                reading.temp_frac
            ),
            Err(Error::NoResponse) => defmt::warn!("DHT11: Ошибка связи с датчиком"),
            Err(Error::Checksum) => defmt::warn!("DHT11: Ошибка контрольной суммы"),
        }

        match sht40.measure(Precision::High, &mut delay) {
            Ok(reading) => defmt::info!(
                "SHT40: hum = {}.{:03} %, temp = {}.{:03} C",
                reading.humidity_milli_percent() / 1000,
                reading.humidity_milli_percent() % 1000,
                reading.temperature_milli_celsius() / 1000,
                reading.temperature_milli_celsius() % 1000,
            ),
            Err(sht4x::Error::Crc) => defmt::warn!("SHT40: Ошибка измерения: контрольная сумма"),
            Err(sht4x::Error::I2c(_)) => defmt::warn!("SHT40: Ошибка измерения: I2C"),
            Err(_) => defmt::warn!("SHT40: Ошибка измерения"),
        }

        // Даем датчику отдохнуть перед следующим циклом измерения
        while timer.now_mks().wrapping_sub(cycle_start) < 2_000_000 {}
    }
}
