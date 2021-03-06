use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
use crate::broker_statement::trades::StockSell;
use crate::commissions::CommissionCalc;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting::table::Cell;
use crate::localities::Country;
use crate::quotes::Quotes;
use crate::util;

pub fn simulate_sell(
    portfolio: &PortfolioConfig, mut statement: BrokerStatement, converter: &CurrencyConverter,
    quotes: &Quotes, positions: &[(String, Option<u32>)],
) -> EmptyResult {
    let mut commission_calc = CommissionCalc::new(statement.broker.commission_spec.clone());

    for (symbol, _) in positions {
        if statement.open_positions.get(symbol).is_none() {
            return Err!("The portfolio has no open {:?} positions", symbol);
        }

        quotes.batch(&symbol);
    }

    for (symbol, quantity) in positions {
        let quantity = *match quantity {
            Some(quantity) => quantity,
            None => match statement.open_positions.get(symbol) {
                Some(quantity) => quantity,
                None => return Err!("The portfolio has no open {:?} positions", symbol),
            }
        };

        statement.emulate_sell(&symbol, quantity, quotes.get(&symbol)?, &mut commission_calc)?;
    }

    statement.process_trades()?;
    let additional_commissions = statement.emulate_commissions(commission_calc);

    let stock_sells = statement.stock_sells.iter()
        .filter(|stock_sell| stock_sell.emulation)
        .cloned().collect::<Vec<_>>();
    assert_eq!(stock_sells.len(), positions.len());

    print_results(stock_sells, additional_commissions, &portfolio.get_tax_country(), converter)
}

#[derive(StaticTable)]
#[table(name="TradesTable")]
struct TradeRow {
    #[column(name="Symbol")]
    symbol: String,
    #[column(name="Quantity")]
    quantity: u32,
    #[column(name="Buy price")]
    buy_price: Cash,
    #[column(name="Sell price")]
    sell_price: Cash,
    #[column(name="Commission")]
    commission: Cash,
    #[column(name="Revenue")]
    revenue: Cash,
    #[column(name="Local revenue")]
    local_revenue: Cash,
    #[column(name="Profit")]
    profit: Cash,
    #[column(name="Local profit")]
    local_profit: Cash,
    #[column(name="Tax to pay")]
    tax_to_pay: Cash,
    #[column(name="Real profit %")]
    real_profit: Cell,
    #[column(name="Real tax %")]
    real_tax: Option<Cell>,
    #[column(name="Real local profit %")]
    real_local_profit: Cell,
}

#[derive(StaticTable)]
#[table(name="FifoTable")]
struct FifoRow {
    #[column(name="Symbol")]
    symbol: Option<String>,
    #[column(name="Quantity")]
    quantity: u32,
    #[column(name="Price")]
    price: Cash,
}

fn print_results(
    stock_sells: Vec<StockSell>, additional_commissions: MultiCurrencyCashAccount,
    country: &Country, converter: &CurrencyConverter
) -> EmptyResult {
    let same_currency = stock_sells.iter().all(|trade| {
        trade.price.currency == country.currency &&
            trade.commission.currency == country.currency
    });

    let mut total_revenue = MultiCurrencyCashAccount::new();
    let mut total_local_revenue = Cash::new(country.currency, dec!(0));

    let mut total_profit = MultiCurrencyCashAccount::new();
    let mut total_local_profit = Cash::new(country.currency, dec!(0));

    for commission in additional_commissions.iter() {
        total_profit.withdraw(commission);
        total_local_profit.sub_convert_assign(
            util::today_trade_conclusion_date(), commission, converter)?;
    }
    let mut total_commission = additional_commissions;

    let mut trades_table = TradesTable::new();
    if same_currency {
        trades_table.hide_local_revenue();
        trades_table.hide_local_profit();
        trades_table.hide_real_tax();
        trades_table.hide_real_local_profit();
    }

    let mut fifo_table = FifoTable::new();

    for trade in stock_sells {
        let details = trade.calculate(&country, &converter)?;
        let mut purchase_cost = Cash::new(trade.price.currency, dec!(0));

        total_commission.deposit(trade.commission);
        total_revenue.deposit(details.revenue);
        total_local_revenue.add_assign(details.local_revenue).unwrap();
        total_profit.deposit(details.profit);
        total_local_profit.add_assign(details.local_profit).unwrap();

        for (index, buy_trade) in details.fifo.iter().enumerate() {
            purchase_cost.add_convert_assign(
                buy_trade.execution_date, buy_trade.price * buy_trade.quantity, converter)?;

            fifo_table.add_row(FifoRow {
                symbol: if index == 0 {
                   Some(trade.symbol.clone())
                } else {
                   None
                },
                quantity: buy_trade.quantity,
                price: buy_trade.price,
            });
        }

        trades_table.add_row(TradeRow {
            symbol: trade.symbol,
            quantity: trade.quantity,
            buy_price: (purchase_cost / trade.quantity).round(),
            sell_price: trade.price,
            commission: trade.commission,
            revenue: details.revenue,
            local_revenue: details.local_revenue,
            profit: details.profit,
            local_profit: details.local_profit,
            tax_to_pay: details.tax_to_pay,
            real_profit: Cell::new_ratio(details.real_profit_ratio),
            real_tax: details.real_tax_ratio.map(Cell::new_ratio),
            real_local_profit: Cell::new_ratio(details.real_local_profit_ratio),
        });
    }

    let tax_to_pay = Cash::new(country.currency, country.tax_to_pay(total_local_profit.amount, None));

    let mut totals = trades_table.add_empty_row();
    totals.set_commission(total_commission);
    totals.set_revenue(total_revenue);
    totals.set_local_revenue(total_local_revenue);
    totals.set_profit(total_profit);
    totals.set_local_profit(total_local_profit);
    totals.set_tax_to_pay(tax_to_pay);

    trades_table.print("Sell simulation results");
    fifo_table.print("FIFO details");

    Ok(())
}
