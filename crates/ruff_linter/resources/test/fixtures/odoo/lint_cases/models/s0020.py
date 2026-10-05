def process(self, orders):
    for order in orders:
        for line in order.order_line:
            products = self.env['product.product'].search([('id', '=', line.product_id.id)])
