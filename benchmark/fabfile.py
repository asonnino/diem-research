from fabric import task
from time import sleep

from benchmark.local import LocalBench
from benchmark.logs import ParseError
from benchmark.utils import Print
from aws.settings import SettingsError
from aws.instance import InstanceManager, AWSError
from aws.remote import Bench, BenchError
# NOTE: Also requires tmux: brew install tmux


@task
def local(ctx, debug=False):
    bench_params = {
        'nodes': 4,
        'txs': 250_000,
        'size': 512,
        'rate': 100_000,
        'duration': 20,
    }
    node_params = {
        'consensus': {
            'timeout_delay': 5000,
            'sync_retry_delay': 10_000
        },
        'mempool': {
            'queue_capacity': 10_000,
            'max_payload_size': 100_000
        }
    }
    try:
        LocalBench(bench_params, node_params).run(debug=debug).print_summary()
    except BenchError as e:
        Print.error(e)


@task
def setup(ctx, nodes=4):
    try:
        InstanceManager.make().create_instances(nodes)
        sleep(1) # Allows to break and re-create an SSH connection.
        Bench(ctx).install()
    except BenchError as e:
        Print.error(e)


@task
def destroy(ctx):
    try:
        InstanceManager.make().terminate_instances()
    except BenchError as e:
        Print.error(e)


@task
def start(ctx):
    try:
        InstanceManager.make().start_instances()
    except BenchError as e:
        Print.error(e)


@task
def stop(ctx):
    try:
        InstanceManager.make().stop_instances()
    except BenchError as e:
        Print.error(e)


@task
def info(ctx):
    try:
        InstanceManager.make().print_info()
    except BenchError as e:
        Print.error(e)


@task
def remote(ctx, debug=False):
    bench_params = {
        'nodes': 4,
        'txs': 1_000_000,
        'size': 512,
        'rate': 0,
        'duration': 350,
        'runs': 1,
    }
    node_params = {
        'consensus': {
            'timeout_delay': 5000,
            'sync_retry_delay': 10_000
        },
        'mempool': {
            'queue_capacity': 10_000_000,
            'max_payload_size': 1_000
        }
    }
    try:
        Bench(ctx).run(bench_params, node_params, debug=debug)
    except BenchError as e:
        Print.error(e)


@task
def kill(ctx):
    try:
        Bench(ctx).kill()
    except BenchError as e:
        Print.error(e)