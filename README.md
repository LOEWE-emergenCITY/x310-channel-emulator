# X310 Channel Emulator

A wideband full-duplex hardware channel emulator utilizing the USRP X310 SDR.

The channel is emulated using a complex FIR filter, which allows modelling of multiple discrete propagation paths up to a certain delay spread and spatial resolution.

The filter coefficients are computed by one of several channel models, specifically designed for UAV Air-Ground communication, taking the relative UAV position as input.

# Requirements

To execute this channel emulator, you require a USRP X310 SDR. 

All necessary software and drivers are set up in a portable Docker container and can be started via docker-compose or plain docker.

Set the IP of your SDR as CHANEM_SDR_IP_ADDR environment variable, e.g. by adapting the example docker-compose.yaml file.

To update the channel model in real-time, position updates and control commands can be streamed in via UDP to port 1337 and port 1341.
