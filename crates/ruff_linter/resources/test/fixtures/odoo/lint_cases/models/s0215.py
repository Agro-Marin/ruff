def _graph_data(self, start_date, end_date):
    query = '''SELECT %(x_query)s as x_value, %(y_query)s as y_value
                FROM %(table)s
                WHERE team_id = %(team_id)s
                AND DATE(%(date_column)s) >= %(start_date)s
                AND DATE(%(date_column)s) <= %(end_date)s
                %(extra_conditions)s
                GROUP BY x_value;'''
    dashboard_graph_model = self._graph_get_model()
    GraphModel = self.env[dashboard_graph_model]
    graph_table = self._graph_get_table(GraphModel)
    extra_conditions = self._extra_sql_conditions()
    where_query = GraphModel._search([])
    from_clause, where_clause, where_clause_params = where_query.get_sql()
    if where_clause:
        extra_conditions += " AND " + where_clause
    query = query % {
        'x_query': self._graph_x_query(),
        'y_query': self._graph_y_query(),
        'table': graph_table,
        'team_id': "%s",
        'date_column': self._graph_date_column(),
        'start_date': "%s",
        'end_date': "%s",
        'extra_conditions': extra_conditions,
    }
    self.env.cr.execute(query, [self.id, start_date, end_date] + where_clause_params)
    return self.env.cr.dictfetchall()
