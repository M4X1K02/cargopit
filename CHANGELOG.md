# Changelog

## [0.4.0](https://github.com/M4X1K02/cargopit/compare/0.3.6...0.4.0) (2026-09-19)


### Features

* extract DiRT Rally 2 haptic telemetry mapping ([#9](https://github.com/M4X1K02/cargopit/issues/9)) ([dfdf8cb](https://github.com/M4X1K02/cargopit/commit/dfdf8cbc56b814b43a828da21dfe31886484da98))
* gate chassis haptics on rolling and real slip ([753e48c](https://github.com/M4X1K02/cargopit/commit/753e48ccd49052fc2c42ae5110089f852b7bd866))
* harden the universal installer for real distros ([7456070](https://github.com/M4X1K02/cargopit/commit/7456070af06c5b77aeca1005670e3ad20b16c12a))
* honor enabled devices and PipeWire streamVolume ([1737fe7](https://github.com/M4X1K02/cargopit/commit/1737fe7d8f04d236ed47266334dc0ce6f4b05ac6))
* map engine rumble across the shaker band with seat FR correction ([df0807a](https://github.com/M4X1K02/cargopit/commit/df0807a7c966ef8de4d3520c341912d5a83dddce))
* share the Moza serial port and keep RPM lights alive ([42603a6](https://github.com/M4X1K02/cargopit/commit/42603a691c6b10512834c0c11dc9cefe3cbfc5aa))
* start simd from play mode when it is not already running ([5bb9238](https://github.com/M4X1K02/cargopit/commit/5bb9238251bfdeb74a52400bceac04adece7b814))
* start the telemetry stack from one manager action ([5ccd942](https://github.com/M4X1K02/cargopit/commit/5ccd942e761a4018aec64ffc62d9e2f502625180))
* USB shaker engine rumble, seat EQ, and chassis gating ([8a5fc31](https://github.com/M4X1K02/cargopit/commit/8a5fc31ddeeb48bcb2849ba56d0f78541e5544f6))


### Bug Fixes

* fail closed on serial and PulseAudio errors ([0617ee5](https://github.com/M4X1K02/cargopit/commit/0617ee52c9f82e6a12dac9595015f3d72457ccaf))
* gate tyre haptics on road speed, not local Y velocity ([28f7e02](https://github.com/M4X1K02/cargopit/commit/28f7e02d0f6408e4a842bfc96091471335097654))
* restore the terminal and stop the loop from signals ([bc21d9f](https://github.com/M4X1K02/cargopit/commit/bc21d9fe36ea26f211244900836c57b59a3bea35))
* skip missing-simd probe after a source install ([3de3e18](https://github.com/M4X1K02/cargopit/commit/3de3e1883485661049baaed11e50858b6298dcc8))
* three crashes found while packaging ([#54](https://github.com/M4X1K02/cargopit/issues/54)) ([8ef05ef](https://github.com/M4X1K02/cargopit/commit/8ef05ef329c1c78bbdf2d9d42a2560b56a795acb))
