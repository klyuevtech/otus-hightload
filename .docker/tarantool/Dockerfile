FROM tarantool/tarantool:latest
WORKDIR /etc/tarantool/
RUN tt init -f
COPY app.lua /etc/tarantool/instances.enabled/auto_leader/app.lua
COPY config.yaml /etc/tarantool/instances.enabled/auto_leader/config.yaml
COPY instances.yml /etc/tarantool/instances.enabled/auto_leader/instances.yml
CMD ["/bin/bash", "-c", "tt start auto_leader && sleep infinity"]
