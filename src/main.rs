#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use cortex_m_rt::entry;
use embedded_hal::digital::{InputPin, OutputPin};
use stm32f4xx_hal::{pac, prelude::*, rcc::Config, timer::Counter};

#[entry]
fn main() -> ! {
    // Получаем доступ к периферии STM32
    let dp = pac::Peripherals::take().unwrap();

    // Настраиваем тактирование: HSI, 16 МГц
    let mut rcc = dp.RCC.freeze(Config::hsi().sysclk(16.MHz()));

    // Получаем доступ к порту GPIOA
    let gpioa = dp.GPIOA.split(&mut rcc);
    // PA0 — пин к DATA DHT11
    let mut dht = gpioa.pa0.into_open_drain_output();
    dht.set_high();

    let mut timer = dp.TIM5.counter_us(&mut rcc);
    timer.start(u32::MAX.micros()).unwrap();

    // Основной бесконечный цикл
    loop {
        let cycle_start = timer.now().ticks();

        if dht_init_measure(&mut dht, &timer) {
            let mut data = [0u8; 5];

            for i in 0..5 {
                let mut byte = 0u8;

                for _ in 0..8 {
                    let t2 = timer.now().ticks();
                    while dht.is_low() && timer.now().ticks().wrapping_sub(t2) < 100 {}

                    let t2 = timer.now().ticks();
                    while dht.is_high() && timer.now().ticks().wrapping_sub(t2) < 150 {}

                    let bit_us = timer.now().ticks().wrapping_sub(t2);
                    byte = (byte << 1) | (bit_us >= 40) as u8;
                }
                data[i] = byte;
            }

            if data[0]
                .wrapping_add(data[1])
                .wrapping_add(data[2])
                .wrapping_add(data[3])
                == data[4]
            {
                defmt::info!("hum = {}%, temp = {}.{} C", data[0], data[2], data[3]);
            } else {
                defmt::warn!("DHT11: Ошибка контрольной суммы");
            }
        } else {
            defmt::warn!("DHT11: Ошибка связи с датчиком");
        }

        // Даем датчику отдохнуть перед следующим циклом измерения
        while timer.now().ticks().wrapping_sub(cycle_start) < 2_000_000 {}
    }
}

fn dht_init_measure(dht: &mut (impl InputPin + OutputPin), timer: &impl MicrosTimer) -> bool {
    // Инициализация последовательности измерения
    let start = timer.now_us();
    dht.set_low().unwrap();
    while timer.now_us().wrapping_sub(start) < 20_000 {}

    let start2 = timer.now_us();
    dht.set_high().unwrap();

    // После того, как отпустили линию, даем время подтяжке поднять линию
    while dht.is_low().unwrap() && timer.now_us().wrapping_sub(start2) < 100 {}

    // Ждем, пока датчик подтянет линию к низкому уровню
    loop {
        if dht.is_low().unwrap() || timer.now_us().wrapping_sub(start2) > 100 {
            break;
        }
    }

    let t0 = timer.now_us();
    while dht.is_low().unwrap() && timer.now_us().wrapping_sub(t0) < 200 {}
    let low_us = timer.now_us().wrapping_sub(t0);

    let t1 = timer.now_us();
    while dht.is_high().unwrap() && timer.now_us().wrapping_sub(t1) < 200 {}
    let high_us = timer.now_us().wrapping_sub(t1);

    if low_us > 40 && low_us < 120 && high_us > 40 && high_us < 120 {
        true
    } else {
        false
    }
}

trait MicrosTimer {
    fn now_us(&self) -> u32;
}

impl<TIM: stm32f4xx_hal::timer::Instance> MicrosTimer for Counter<TIM, 1_000_000> {
    fn now_us(&self) -> u32 {
        self.now().ticks()
    }
}
