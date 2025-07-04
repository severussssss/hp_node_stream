# Generated by the gRPC Python protocol compiler plugin. DO NOT EDIT!
"""Client and server classes corresponding to protobuf-defined services."""
import grpc
import warnings

import orderbook_pb2 as orderbook__pb2

GRPC_GENERATED_VERSION = '1.73.0'
GRPC_VERSION = grpc.__version__
_version_not_supported = False

try:
    from grpc._utilities import first_version_is_lower
    _version_not_supported = first_version_is_lower(GRPC_VERSION, GRPC_GENERATED_VERSION)
except ImportError:
    _version_not_supported = True

if _version_not_supported:
    raise RuntimeError(
        f'The grpc package installed is at version {GRPC_VERSION},'
        + f' but the generated code in orderbook_pb2_grpc.py depends on'
        + f' grpcio>={GRPC_GENERATED_VERSION}.'
        + f' Please upgrade your grpc module to grpcio>={GRPC_GENERATED_VERSION}'
        + f' or downgrade your generated code using grpcio-tools<={GRPC_VERSION}.'
    )


class OrderbookServiceStub(object):
    """Missing associated documentation comment in .proto file."""

    def __init__(self, channel):
        """Constructor.

        Args:
            channel: A grpc.Channel.
        """
        self.SubscribeOrderbook = channel.unary_stream(
                '/orderbook.OrderbookService/SubscribeOrderbook',
                request_serializer=orderbook__pb2.SubscribeRequest.SerializeToString,
                response_deserializer=orderbook__pb2.OrderbookSnapshot.FromString,
                _registered_method=True)
        self.GetOrderbook = channel.unary_unary(
                '/orderbook.OrderbookService/GetOrderbook',
                request_serializer=orderbook__pb2.GetOrderbookRequest.SerializeToString,
                response_deserializer=orderbook__pb2.OrderbookSnapshot.FromString,
                _registered_method=True)
        self.GetMarkets = channel.unary_unary(
                '/orderbook.OrderbookService/GetMarkets',
                request_serializer=orderbook__pb2.GetMarketsRequest.SerializeToString,
                response_deserializer=orderbook__pb2.GetMarketsResponse.FromString,
                _registered_method=True)


class OrderbookServiceServicer(object):
    """Missing associated documentation comment in .proto file."""

    def SubscribeOrderbook(self, request, context):
        """Subscribe to orderbook updates
        """
        context.set_code(grpc.StatusCode.UNIMPLEMENTED)
        context.set_details('Method not implemented!')
        raise NotImplementedError('Method not implemented!')

    def GetOrderbook(self, request, context):
        """Get current orderbook snapshot
        """
        context.set_code(grpc.StatusCode.UNIMPLEMENTED)
        context.set_details('Method not implemented!')
        raise NotImplementedError('Method not implemented!')

    def GetMarkets(self, request, context):
        """Get available markets
        """
        context.set_code(grpc.StatusCode.UNIMPLEMENTED)
        context.set_details('Method not implemented!')
        raise NotImplementedError('Method not implemented!')


def add_OrderbookServiceServicer_to_server(servicer, server):
    rpc_method_handlers = {
            'SubscribeOrderbook': grpc.unary_stream_rpc_method_handler(
                    servicer.SubscribeOrderbook,
                    request_deserializer=orderbook__pb2.SubscribeRequest.FromString,
                    response_serializer=orderbook__pb2.OrderbookSnapshot.SerializeToString,
            ),
            'GetOrderbook': grpc.unary_unary_rpc_method_handler(
                    servicer.GetOrderbook,
                    request_deserializer=orderbook__pb2.GetOrderbookRequest.FromString,
                    response_serializer=orderbook__pb2.OrderbookSnapshot.SerializeToString,
            ),
            'GetMarkets': grpc.unary_unary_rpc_method_handler(
                    servicer.GetMarkets,
                    request_deserializer=orderbook__pb2.GetMarketsRequest.FromString,
                    response_serializer=orderbook__pb2.GetMarketsResponse.SerializeToString,
            ),
    }
    generic_handler = grpc.method_handlers_generic_handler(
            'orderbook.OrderbookService', rpc_method_handlers)
    server.add_generic_rpc_handlers((generic_handler,))
    server.add_registered_method_handlers('orderbook.OrderbookService', rpc_method_handlers)


 # This class is part of an EXPERIMENTAL API.
class OrderbookService(object):
    """Missing associated documentation comment in .proto file."""

    @staticmethod
    def SubscribeOrderbook(request,
            target,
            options=(),
            channel_credentials=None,
            call_credentials=None,
            insecure=False,
            compression=None,
            wait_for_ready=None,
            timeout=None,
            metadata=None):
        return grpc.experimental.unary_stream(
            request,
            target,
            '/orderbook.OrderbookService/SubscribeOrderbook',
            orderbook__pb2.SubscribeRequest.SerializeToString,
            orderbook__pb2.OrderbookSnapshot.FromString,
            options,
            channel_credentials,
            insecure,
            call_credentials,
            compression,
            wait_for_ready,
            timeout,
            metadata,
            _registered_method=True)

    @staticmethod
    def GetOrderbook(request,
            target,
            options=(),
            channel_credentials=None,
            call_credentials=None,
            insecure=False,
            compression=None,
            wait_for_ready=None,
            timeout=None,
            metadata=None):
        return grpc.experimental.unary_unary(
            request,
            target,
            '/orderbook.OrderbookService/GetOrderbook',
            orderbook__pb2.GetOrderbookRequest.SerializeToString,
            orderbook__pb2.OrderbookSnapshot.FromString,
            options,
            channel_credentials,
            insecure,
            call_credentials,
            compression,
            wait_for_ready,
            timeout,
            metadata,
            _registered_method=True)

    @staticmethod
    def GetMarkets(request,
            target,
            options=(),
            channel_credentials=None,
            call_credentials=None,
            insecure=False,
            compression=None,
            wait_for_ready=None,
            timeout=None,
            metadata=None):
        return grpc.experimental.unary_unary(
            request,
            target,
            '/orderbook.OrderbookService/GetMarkets',
            orderbook__pb2.GetMarketsRequest.SerializeToString,
            orderbook__pb2.GetMarketsResponse.FromString,
            options,
            channel_credentials,
            insecure,
            call_credentials,
            compression,
            wait_for_ready,
            timeout,
            metadata,
            _registered_method=True)
