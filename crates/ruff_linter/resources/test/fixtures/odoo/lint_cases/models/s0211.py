def _get_seen_list(self):
    target = self.env[self.mailing_model_real]
    query = "SELECT s.email FROM mailing_trace s JOIN %(target)s t ON (s.res_id = t.id)"
    if self.ab_testing_enabled:
        query += " AND s.campaign_id = %%(mailing_campaign_id)s"
    else:
        query += " AND s.mass_mailing_id = %%(mailing_id)s"
    query = query % {"target": target._table}
    self.env.cr.execute(query, {"mailing_id": self.id})
