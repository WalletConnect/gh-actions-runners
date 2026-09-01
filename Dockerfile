FROM myoung34/github-runner:2.337.0-ubuntu-noble

# modify actions runner binaries to allow custom cache server implementation
# https://gha-cache-server.falcondev.io/getting-started
RUN sed -i 's/\x41\x00\x43\x00\x54\x00\x49\x00\x4F\x00\x4E\x00\x53\x00\x5F\x00\x43\x00\x41\x00\x43\x00\x48\x00\x45\x00\x5F\x00\x55\x00\x52\x00\x4C\x00/\x41\x00\x43\x00\x54\x00\x49\x00\x4F\x00\x4E\x00\x53\x00\x5F\x00\x43\x00\x41\x00\x43\x00\x48\x00\x45\x00\x5F\x00\x4F\x00\x52\x00\x4C\x00/g' /actions-runner/bin/Runner.Worker.dll

# Node 20+ is required: Playwright dropped Node 18 support in 1.62.0 (2026-07-24).
# Ubuntu noble's apt `npm` pulls Node 18, so install Node from NodeSource instead.
ARG NODE_MAJOR=22
ARG PLAYWRIGHT_VERSION=1.62.1

# Install Playwright System Dependencies
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl gnupg \
  && mkdir -p /etc/apt/keyrings \
  && curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
       | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg \
  && echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_${NODE_MAJOR}.x nodistro main" \
       > /etc/apt/sources.list.d/nodesource.list \
  && apt-get update \
  && apt-get install -y --no-install-recommends nodejs \
  && npm install -g "playwright@${PLAYWRIGHT_VERSION}" \
  && playwright install-deps \
  && npm cache clean --force \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/* /root/.npm /tmp/*

CMD timeout $TIMEOUT ./bin/Runner.Listener run --startuptype service
