#!/usr/bin/env python3

from time import sleep
import argparse
import numpy as np
import signal
import socket
import struct
import sys
import uhd

def get_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rx-gain", type=float, default=20.0, help="RX Gain")
    parser.add_argument("--tx-gain", type=float, default=20.0, help="TX Gain")
    parser.add_argument("--freq", type=float, default=2.45e9, help="Center Frequency")
    parser.add_argument("--bw", type=float, default=200e6, help="Radio Bandwidth")
    parser.add_argument("--sample-rate", type=float, default=200e6, help="Sample Rate")
    parser.add_argument("--udp-port", type=int, default=1338, help="Local UDP Port")
    return parser.parse_args()

def signal_handler(sig, frame):
    print('Stop Streaming')
    radio0.issue_stream_cmd(uhd.rfnoc.lib.types.stream_cmd(uhd.rfnoc.lib.types.stream_mode.stop_cont), 0)
    radio1.issue_stream_cmd(uhd.rfnoc.lib.types.stream_cmd(uhd.rfnoc.lib.types.stream_mode.stop_cont), 0)
    # f.close()
    sys.exit(0)


if __name__ == '__main__':
    # global f
    # f = open("/tmp/log_sdr_py.txt", 'w')
    # sys.stdout = f

    args = get_args()
    print(args)

    # Create graph
    graph = uhd.rfnoc.RfnocGraph("addr=10.193.0.69")

    # Connect blocks
    graph.connect("0/Radio#0", 0, "0/DDC#0", 0, False)
    graph.connect("0/DDC#0", 0, "0/SplitStream#0", 0, False)
    graph.connect("0/SplitStream#0", 0, "0/FIR#0", 0, False)
    graph.connect("0/SplitStream#0", 1, "0/FIR#1", 0, False)
    graph.connect("0/FIR#0", 0, "0/AddCom#0", 0, False)
    graph.connect("0/FIR#1", 0, "0/AddCom#0", 1, False)
    graph.connect("0/AddCom#0", 0, "0/DUC#1", 0, False)
    graph.connect("0/DUC#1", 0, "0/Radio#1", 0, False)

    graph.connect("0/Radio#1", 0, "0/DDC#1", 0, False)
    graph.connect("0/DDC#1", 0, "0/SplitStream#1", 0, False)
    graph.connect("0/SplitStream#1", 0, "0/FIR#2", 0, False)
    graph.connect("0/SplitStream#1", 1, "0/FIR#3", 0, False)
    graph.connect("0/FIR#2", 0, "0/AddCom#1", 0, False)
    graph.connect("0/FIR#3", 0, "0/AddCom#1", 1, False)
    graph.connect("0/AddCom#1", 0, "0/DUC#0", 0, True)
    graph.connect("0/DUC#0", 0, "0/Radio#0", 0, False)

    # Get Block controllers
    global radio0
    global radio1
    radio0 = uhd.rfnoc.RadioControl(graph.get_block("0/Radio#0"))
    radio1 = uhd.rfnoc.RadioControl(graph.get_block("0/Radio#1"))
    ddc0 = uhd.rfnoc.DdcBlockControl(graph.get_block("0/DDC#0"))
    ddc1 = uhd.rfnoc.DdcBlockControl(graph.get_block("0/DDC#1"))
    duc0 = uhd.rfnoc.DucBlockControl(graph.get_block("0/DUC#0"))
    duc1 = uhd.rfnoc.DucBlockControl(graph.get_block("0/DUC#1"))
    fir0 = uhd.rfnoc.FirFilterBlockControl(graph.get_block("0/FIR#0"))
    fir1 = uhd.rfnoc.FirFilterBlockControl(graph.get_block("0/FIR#1"))
    fir2 = uhd.rfnoc.FirFilterBlockControl(graph.get_block("0/FIR#2"))
    fir3 = uhd.rfnoc.FirFilterBlockControl(graph.get_block("0/FIR#3"))

    # Set block properties
    # Radio0
    radio0.set_rx_frequency(args.freq, 0)
    radio0.set_rx_gain(args.rx_gain, 0)
    radio0.set_rx_antenna("RX2", 0)
    radio0.set_rx_bandwidth(args.bw, 0)
    radio0.enable_rx_timestamps(False, 0)

    radio0.set_tx_frequency(args.freq, 0)
    radio0.set_tx_gain(args.tx_gain, 0)
    radio0.set_tx_antenna("TX/RX", 0)
    radio0.set_tx_bandwidth(args.bw, 0)

    radio0.set_rate(200e6)
    radio0.set_properties("spp:0=128")

    # Radio1
    radio1.set_rx_frequency(args.freq, 0)
    radio1.set_rx_gain(args.rx_gain, 0)
    radio1.set_rx_antenna("RX2", 0)
    radio1.set_rx_bandwidth(args.bw, 0)
    radio1.enable_rx_timestamps(False, 0)

    radio1.set_tx_frequency(args.freq, 0)
    radio1.set_tx_gain(args.tx_gain, 0)
    radio1.set_tx_antenna("TX/RX", 0)
    radio1.set_tx_bandwidth(args.bw, 0)

    radio1.set_rate(200e6)
    radio1.set_properties("spp:0=128")

    #DDC0
    ddc0.set_input_rate(200e6, 0)
    ddc0.set_output_rate(args.sample_rate, 0)
    print("DDC output rate wanted: " + str(args.sample_rate) + " got: " + str(ddc0.get_output_rate(0)))
    print("DDC input rate wanted: " + str(200e6) + " got: " + str(ddc0.get_input_rate(0)))

    # DDC1
    ddc1.set_input_rate(200e6, 0)
    ddc1.set_output_rate(args.sample_rate, 0)

    # DUC0
    duc0.set_output_rate(200e6, 0)
    duc0.set_input_rate(args.sample_rate, 0)
    print("DUC input rate wanted: " + str(args.sample_rate) + " got: " + str(duc0.get_input_rate(0)))
    print("DUC output rate wanted: " + str(200e6) + " got: " + str(duc0.get_output_rate(0)))

    #DUC1
    duc1.set_output_rate(200e6, 0)
    duc1.set_input_rate(args.sample_rate, 0)

    # FIR0
    fir0.set_coefficients([32767//4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])

    # FIR1
    fir1.set_coefficients([32767//4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])

    # FIR2
    fir2.set_coefficients([32767//4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])

    # FIR3
    fir3.set_coefficients([32767//4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])


    # Commit blocks
    for edge in graph.enumerate_active_connections():
        print(edge.to_string())

    graph.commit()

    sleep(1)

    streamcmd = uhd.rfnoc.lib.types.stream_cmd(uhd.rfnoc.lib.types.stream_mode.start_cont)
    streamcmd.stream_now = True
    radio0.issue_stream_cmd(streamcmd, 0)
    radio1.issue_stream_cmd(streamcmd, 0)

    signal.signal(signal.SIGINT, signal_handler)

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", int(args.udp_port)))

    while True:
        data, addr = sock.recvfrom(1024)
        if len(data) == 2 * 2 * 41:
            taps = struct.unpack('!' + 'h' * 41 * 2, data)
            print("updating taps (real): " + str(list(taps[:41])))
            print("updating taps (imag): " + str(list(taps[41:])))
            fir0.set_coefficients(list(taps[:41]))
            fir2.set_coefficients(list(taps[:41]))
            fir1.set_coefficients(list(taps[41:]))
            fir3.set_coefficients(list(taps[41:]))
        else:
            assert chr(data[0]) == "F"
            data = data[1:]
            assert(len(data) == 4)
            (frequency, ) = struct.unpack('!' + 'l', data)
            radio0.set_rx_frequency(frequency, 0)
            radio0.set_tx_frequency(frequency, 0)
            radio1.set_rx_frequency(frequency, 0)
            radio1.set_tx_frequency(frequency, 0)



