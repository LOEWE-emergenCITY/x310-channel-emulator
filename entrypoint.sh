#!/bin/bash
#export CHANEM_SDR_IP_ADDR=$(uhd_find_devices --args="type=x300" | grep addr | sed -r 's/^.*addr: (.*)/\1/')
echo looking for X310 @ $CHANEM_SDR_IP_ADDR
python /usr/local/src/uhd/firmware/usrp3/x300/x300_debug.py --addr=$CHANEM_SDR_IP_ADDR --poke=0x100058 --data=1 ; sleep 15 ; sdr.py &> /shared/log_sdr.txt & sleep 20 ; unshare --net /lib/systemd/systemd-udevd --daemon ; udevadm trigger ; chanem &> /shared/log_chanem.txt