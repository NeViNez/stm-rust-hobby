#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use cortex_m_rt::entry;
use stm32f4xx_hal::{pac, prelude::*, rcc::Config};

use sensors::dht11::{Dht11, Error, MicrosTimer};

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

    let mut timer = dp.TIM5.counter_us(&mut rcc);
    timer.start(u32::MAX.micros()).unwrap();

    let mut dht11 = Dht11::new(dht_pin, &timer);

    // Основной бесконечный цикл
    loop {
        let cycle_start = timer.now_mks();

        match dht11.read() {
            Ok(reading) => defmt::info!(
                "hum = {}%, temp = {}.{} C",
                reading.humidity,
                reading.temp_int,
                reading.temp_frac
            ),
            Err(Error::NoResponse) => defmt::warn!("DHT11: Ошибка связи с датчиком"),
            Err(Error::Checksum) => defmt::warn!("DHT11: Ошибка контрольной суммы"),
        }

        // Даем датчику отдохнуть перед следующим циклом измерения
        while timer.now_mks().wrapping_sub(cycle_start) < 100_000_000 {}
    }
}
