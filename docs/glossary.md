# Glossary

## Misc

- ISIS pulse: for a given target station, the delivery of a proton pulse
- Frame: the duration of the neutron or muon peak, the time in which data is acquired
- Status packet: a small packet containing status information that is syncronised between all digitisers/DAQs

## External Components

- IBEX: UI for the new instrument control system
- [EPICS](https://epics-controls.org/): control system framework upon which the new instrment control system is built
- SECI: UI for the old instrument control system
- ICP: sort of legacy, sort of not (very few really know) software that controls the instrument
- Digitiser: a rackmount unit containing 4 DAQs, baseboard and power supplies
- DAQ: an 8 channel data acquisition board, (incorrectly) referred to as a "digitiser" in most of this repository

## Technologies

- [(Apache) Kafka](https://kafka.apache.org/): an event streaming platform
- [Redpanda](https://redpanda.com/): an implementation of Apache Kafka, offering higher performance
- [Google flatbuffers](https://github.com/google/flatbuffers): a serialisation library
- [HDF5](https://www.hdfgroup.org/solutions/hdf5/): a hierarchical data file format
- [NeXus](https://www.nexusformat.org/): the de-facto data format for neutron and muon data, a schema on top of HDF5
- [TDengine](https://tdengine.com/): a high performance time series database
