FROM alpine:3.22

ENV MOZ_HEADLESS=1
RUN apk update
RUN apk add --upgrade geckodriver firefox