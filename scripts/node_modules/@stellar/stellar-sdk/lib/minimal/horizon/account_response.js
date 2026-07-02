"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true
});
exports.AccountResponse = void 0;
var _stellarBase = require("@stellar/stellar-base");
function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
function _classCallCheck(a, n) { if (!(a instanceof n)) throw new TypeError("Cannot call a class as a function"); }
function _defineProperties(e, r) { for (var t = 0; t < r.length; t++) { var o = r[t]; o.enumerable = o.enumerable || !1, o.configurable = !0, "value" in o && (o.writable = !0), Object.defineProperty(e, _toPropertyKey(o.key), o); } }
function _createClass(e, r, t) { return r && _defineProperties(e.prototype, r), t && _defineProperties(e, t), Object.defineProperty(e, "prototype", { writable: !1 }), e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == _typeof(i) ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != _typeof(t) || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r || "default"); if ("object" != _typeof(i)) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
var AccountResponse = exports.AccountResponse = function () {
  function AccountResponse(response) {
    _classCallCheck(this, AccountResponse);
    this._baseAccount = new _stellarBase.Account(response.account_id, response.sequence);
    this.effects = response.effects;
    this.offers = response.offers;
    this.operations = response.operations;
    this.payments = response.payments;
    this.trades = response.trades;
    this.data = response.data;
    this.transactions = response.transactions;
    this.id = response.id;
    this.paging_token = response.paging_token;
    this.account_id = response.account_id;
    this.sequence = response.sequence;
    this.sequence_ledger = response.sequence_ledger;
    this.sequence_time = response.sequence_time;
    this.subentry_count = response.subentry_count;
    this.home_domain = response.home_domain;
    this.inflation_destination = response.inflation_destination;
    this.last_modified_ledger = response.last_modified_ledger;
    this.last_modified_time = response.last_modified_time;
    this.thresholds = response.thresholds;
    this.flags = response.flags;
    this.balances = response.balances;
    this.signers = response.signers;
    this.data_attr = response.data_attr;
    this.sponsor = response.sponsor;
    this.num_sponsoring = response.num_sponsoring;
    this.num_sponsored = response.num_sponsored;
  }
  return _createClass(AccountResponse, [{
    key: "accountId",
    value: function accountId() {
      return this._baseAccount.accountId();
    }
  }, {
    key: "sequenceNumber",
    value: function sequenceNumber() {
      return this._baseAccount.sequenceNumber();
    }
  }, {
    key: "incrementSequenceNumber",
    value: function incrementSequenceNumber() {
      this._baseAccount.incrementSequenceNumber();
      this.sequence = this._baseAccount.sequenceNumber();
    }
  }]);
}();