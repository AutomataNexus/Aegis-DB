module.exports = {
  apps: [
    {
      name: 'aegis-dashboard',
      script: '/opt/Aegis-DB/target/release/aegis-server',
      args: '--port 9090 --node-name Dashboard --peers 127.0.0.1:9091,127.0.0.1:7001',
      cwd: '/opt/Aegis-DB',
      env: {
        RUST_LOG: 'info'
      },
      autorestart: true,
      max_restarts: 10,
      restart_delay: 2000
    },
    {
      name: 'aegis-axonml',
      script: '/opt/Aegis-DB/target/release/aegis-server',
      args: '--port 7001 --node-name AxonML --peers 127.0.0.1:9090,127.0.0.1:9091 --data-dir /opt/AxonML/data/aegis-db',
      cwd: '/opt/AxonML',
      env: {
        RUST_LOG: 'info'
      },
      autorestart: true,
      max_restarts: 10,
      restart_delay: 2000
    },
    {
      name: 'aegis-nexusscribe',
      script: '/opt/Aegis-DB/target/release/aegis-server',
      args: '--port 9091 --node-name NexusScribe --peers 127.0.0.1:9090,127.0.0.1:7001 --data-dir /opt/NexusScribe/data/aegis-db',
      cwd: '/opt/NexusScribe',
      env: {
        RUST_LOG: 'info'
      },
      autorestart: true,
      max_restarts: 10,
      restart_delay: 2000
    }
  ]
};
