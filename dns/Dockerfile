FROM alpine:3.21

RUN apk add --no-cache unbound tini && \
    mkdir -p /etc/unbound && \
    chown -R unbound:unbound /etc/unbound

COPY unbound.conf /etc/unbound/unbound.conf
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

USER unbound

EXPOSE 53/tcp 53/udp

ENTRYPOINT ["tini", "--"]
CMD ["/entrypoint.sh"]
