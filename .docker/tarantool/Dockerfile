FROM tarantool/tarantool:latest
COPY init.lua /opt/tarantool
ENV TT_APP_NAME=app
CMD ["tarantool","/opt/tarantool/init.lua"]