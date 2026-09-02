{{ config(materialized='incremental', unique_key='id') }}

{# Was missing (SCA-533): the merge used to double count. #}

-- Was missing (SCA-700): this used to silently drop nulls.
with src as (
    select
        id,
        'a -- not a comment' as literal_dashes,
        '/* not a comment */' as literal_block,
        {{ "-- not a comment either" }} as jinja_expr,
        dt
    from {{ ref('raw_events') }}
    {% if is_incremental() %}
    where dt > (select max(dt) from {{ this }})
    {% endif %}
),
/* This should work but I'm not sure why the dedupe fails sometimes. */
final as (
    select * from src
    qualify row_number() over (partition by id order by dt desc) = 1
)
-- Sorted DESC because the caller assumes the first row is newest.
select * from final order by dt desc
