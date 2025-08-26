FROM myoung34/github-runner:2.328.0-ubuntu-noble

# modify actions runner binaries to allow custom cache server implementation
# https://gha-cache-server.falcondev.io/getting-started
RUN sed -i 's/\x41\x00\x43\x00\x54\x00\x49\x00\x4F\x00\x4E\x00\x53\x00\x5F\x00\x43\x00\x41\x00\x43\x00\x48\x00\x45\x00\x5F\x00\x55\x00\x52\x00\x4C\x00/\x41\x00\x43\x00\x54\x00\x49\x00\x4F\x00\x4E\x00\x53\x00\x5F\x00\x43\x00\x41\x00\x43\x00\x48\x00\x45\x00\x5F\x00\x4F\x00\x52\x00\x4C\x00/g' /actions-runner/bin/Runner.Worker.dll

# Install Playwright System Dependencies
RUN apt-get update && apt-get install -y npm
RUN npm install -g playwright && playwright install-deps

CMD timeout $TIMEOUT ./bin/Runner.Listener run --startuptype service
