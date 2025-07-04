# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# NO CHECKED-IN PROTOBUF GENCODE
# source: orderbook.proto
# Protobuf Python Version: 6.31.0
"""Generated protocol buffer code."""
from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import runtime_version as _runtime_version
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder
_runtime_version.ValidateProtobufRuntimeVersion(
    _runtime_version.Domain.PUBLIC,
    6,
    31,
    0,
    '',
    'orderbook.proto'
)
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()




DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(b'\n\x0forderbook.proto\x12\torderbook\"&\n\x10SubscribeRequest\x12\x12\n\nmarket_ids\x18\x01 \x03(\r\"7\n\x13GetOrderbookRequest\x12\x11\n\tmarket_id\x18\x01 \x01(\r\x12\r\n\x05\x64\x65pth\x18\x02 \x01(\r\"\x13\n\x11GetMarketsRequest\"<\n\x12GetMarketsResponse\x12&\n\x07markets\x18\x01 \x03(\x0b\x32\x15.orderbook.MarketInfo\"?\n\nMarketInfo\x12\x11\n\tmarket_id\x18\x01 \x01(\r\x12\x0e\n\x06symbol\x18\x02 \x01(\t\x12\x0e\n\x06\x61\x63tive\x18\x03 \x01(\x08\"B\n\nPriceLevel\x12\r\n\x05price\x18\x01 \x01(\x01\x12\x10\n\x08quantity\x18\x02 \x01(\x01\x12\x13\n\x0border_count\x18\x03 \x01(\r\"\xa8\x01\n\x11OrderbookSnapshot\x12\x11\n\tmarket_id\x18\x01 \x01(\r\x12\x0e\n\x06symbol\x18\x02 \x01(\t\x12\x14\n\x0ctimestamp_us\x18\x03 \x01(\x04\x12\x10\n\x08sequence\x18\x04 \x01(\x04\x12#\n\x04\x62ids\x18\x05 \x03(\x0b\x32\x15.orderbook.PriceLevel\x12#\n\x04\x61sks\x18\x06 \x03(\x0b\x32\x15.orderbook.PriceLevel2\xfe\x01\n\x10OrderbookService\x12Q\n\x12SubscribeOrderbook\x12\x1b.orderbook.SubscribeRequest\x1a\x1c.orderbook.OrderbookSnapshot0\x01\x12L\n\x0cGetOrderbook\x12\x1e.orderbook.GetOrderbookRequest\x1a\x1c.orderbook.OrderbookSnapshot\x12I\n\nGetMarkets\x12\x1c.orderbook.GetMarketsRequest\x1a\x1d.orderbook.GetMarketsResponseb\x06proto3')

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, 'orderbook_pb2', _globals)
if not _descriptor._USE_C_DESCRIPTORS:
  DESCRIPTOR._loaded_options = None
  _globals['_SUBSCRIBEREQUEST']._serialized_start=30
  _globals['_SUBSCRIBEREQUEST']._serialized_end=68
  _globals['_GETORDERBOOKREQUEST']._serialized_start=70
  _globals['_GETORDERBOOKREQUEST']._serialized_end=125
  _globals['_GETMARKETSREQUEST']._serialized_start=127
  _globals['_GETMARKETSREQUEST']._serialized_end=146
  _globals['_GETMARKETSRESPONSE']._serialized_start=148
  _globals['_GETMARKETSRESPONSE']._serialized_end=208
  _globals['_MARKETINFO']._serialized_start=210
  _globals['_MARKETINFO']._serialized_end=273
  _globals['_PRICELEVEL']._serialized_start=275
  _globals['_PRICELEVEL']._serialized_end=341
  _globals['_ORDERBOOKSNAPSHOT']._serialized_start=344
  _globals['_ORDERBOOKSNAPSHOT']._serialized_end=512
  _globals['_ORDERBOOKSERVICE']._serialized_start=515
  _globals['_ORDERBOOKSERVICE']._serialized_end=769
# @@protoc_insertion_point(module_scope)
