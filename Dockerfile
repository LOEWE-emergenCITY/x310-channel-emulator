FROM        ubuntu:20.04

ENV DEBIAN_FRONTEND noninteractive

RUN apt-get update
RUN apt-get -y install -q \
      build-essential \
      git \
      python \
      python3-dev \
      python3-pip \
      curl

RUN apt-get -y install -q \
      libboost-all-dev \
      libusb-1.0-0-dev \
      libudev-dev \
      python3-mako \
      doxygen \
      python3-docutils \
      cmake \
      python3-requests \
      python3-numpy \
      dpdk \
      libdpdk-dev

RUN mkdir -p /usr/local/src
RUN git clone https://github.com/LOEWE-emergenCITY/uhd.git /usr/local/src/uhd
RUN cd /usr/local/src/uhd/ && git checkout UHD-4.0-complex-fir-filter
RUN mkdir -p /usr/local/src/uhd/host/build
WORKDIR      /usr/local/src/uhd/host/build

RUN          cmake .. -DUHD_RELEASE_MODE=release -DCMAKE_INSTALL_PREFIX=/usr
RUN          make -j $MAKEWIDTH
RUN          make install
RUN          uhd_images_downloader
WORKDIR      /

RUN mkdir -p /usr/local/bin

RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN apt-get -y install -q pkg-config libudev-dev
COPY chanem /usr/local/src/chanem
COPY channel_models /usr/local/src/channel_models
WORKDIR /usr/local/src/chanem
RUN cargo install --bins --path . --root /usr/local

ENV UHD_IMAGES_DIR /usr/share/uhd/images/

COPY sdr.py /usr/local/bin
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
WORKDIR /usr/local/bin/

ENTRYPOINT bash -c " bash entrypoint.sh & bash "

