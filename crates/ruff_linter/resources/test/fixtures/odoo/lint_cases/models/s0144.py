if TYPE_CHECKING:
    try:
        from odoo.orm.query import Query
    except ImportError:
        Query = object
